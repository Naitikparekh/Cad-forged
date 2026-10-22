//! Small dialogs: save as, about, help, QuickCalc, purge, layer states, alias editor.
use super::Out;
use crate::app::CadForgeApp;
use eframe::egui::{self, vec2, Color32, RichText};
use std::collections::BTreeSet;

impl CadForgeApp {
    pub(super) fn ui_save_as(&mut self, ui: &mut egui::Ui, name: &mut String) -> Out {
        ui.label(RichText::new("File name (use .dxf for DXF, otherwise CadForge JSON)").color(self.t.text2));
        let r = ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.trim().is_empty() {
            return Out::Ok;
        }
        if !r.has_focus() && !self.save_as_focused {
            r.request_focus();
            self.save_as_focused = true;
        }
        let o = self.ok_cancel(ui, Some("Save"), Some("Cancel"), &[]);
        if o != Out::Open {
            self.save_as_focused = false;
        }
        if o == Out::Ok && name.trim().is_empty() {
            return Out::Open;
        }
        o
    }

    pub(super) fn ui_about(&mut self, ui: &mut egui::Ui) -> Out {
        let t = self.t;
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(64.0, 64.0), egui::Sense::hover());
            crate::chrome::logo(ui.painter(), r.center(), 64.0);
            ui.label(RichText::new("CadForge 2.0").heading());
            ui.label(RichText::new("A 2D drafting and 3D solid modelling application with an AutoCAD-style interface and command set.").color(t.text2));
            let (cmds, aliases) = {
                let a = self.h.borrow();
                (a.registry.names().len(), a.registry.names().len())
            };
            let _ = aliases;
            ui.label(RichText::new(format!("{cmds} commands · written in Rust · runs natively and in the browser.")).small().color(t.text3));
            ui.label(RichText::new(format!("Theme: {}", if t.dark { "Dark" } else { "Light" })).small().color(t.text3));
        });
        self.ok_cancel(ui, None, Some("Close"), &[])
    }

    pub(super) fn ui_help(&mut self, ui: &mut egui::Ui, s: &mut HelpState) -> Out {
        let t = self.t;
        self.tab_strip(ui, &["Commands", "Keyboard", "Command line"], &mut s.tab);
        match s.tab {
            0 => {
                ui.horizontal(|ui| {
                    ui.label("Search");
                    ui.add(egui::TextEdit::singleline(&mut s.query).hint_text("Search commands").desired_width(280.0));
                });
                let q = s.query.trim().to_lowercase();
                let defs: Vec<(String, String, String, String, String)> = {
                    let a = self.h.borrow();
                    let mut v: Vec<_> = a
                        .registry
                        .defs()
                        .filter(|d| !d.name.starts_with('_') && !d.name.starts_with("OSNAP_") && !d.title.is_empty() && (q.is_empty() || d.name.to_lowercase().contains(&q) || d.title.to_lowercase().contains(&q) || d.desc.to_lowercase().contains(&q) || a.registry.alias_of(&d.name).is_some_and(|al| al.to_lowercase() == q)))
                        .map(|d| (d.name.clone(), if d.icon.is_empty() { d.name.to_lowercase() } else { d.icon.clone() }, a.registry.alias_of(&d.name).unwrap_or_default(), if d.desc.is_empty() { d.title.clone() } else { d.desc.clone() }, d.title.clone()))
                        .collect();
                    v.sort();
                    v.truncate(80);
                    v
                };
                let mut run: Option<String> = None;
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for (name, icon, alias, desc, _) in &defs {
                        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), egui::Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 2.0, t.hover);
                        }
                        self.icons.paint(ui.painter(), icon, egui::Rect::from_center_size(r.left_center() + vec2(14.0, 0.0), vec2(18.0, 18.0)), &t, crate::icons::Tint::Normal);
                        ui.painter().text(r.left_center() + vec2(32.0, 0.0), egui::Align2::LEFT_CENTER, name, egui::FontId::new(12.0, egui::FontFamily::Name("bold".into())), t.text);
                        ui.painter().text(r.left_center() + vec2(150.0, 0.0), egui::Align2::LEFT_CENTER, alias, egui::FontId::monospace(11.5), t.accent2);
                        let d: String = desc.chars().take(70).collect();
                        ui.painter().text(r.left_center() + vec2(210.0, 0.0), egui::Align2::LEFT_CENTER, d, egui::FontId::proportional(11.5), t.text3);
                        if resp.clicked() {
                            run = Some(name.clone());
                        }
                    }
                });
                if let Some(n) = run {
                    self.dialogs.clear();
                    self.run_command(&n, cadforge_engine::runtime::Args::none());
                    return Out::Cancel;
                }
            }
            1 => {
                let keys = [
                    ("F1", "Help"),
                    ("F2", "Text window"),
                    ("F3", "Object snap"),
                    ("F7", "Grid"),
                    ("F8", "Ortho"),
                    ("F9", "Snap"),
                    ("F10", "Polar tracking"),
                    ("F11", "Object snap tracking"),
                    ("F12", "Dynamic input"),
                    ("Ctrl+N / O / S", "New / Open / Save"),
                    ("Ctrl+Z / Y", "Undo / Redo"),
                    ("Ctrl+C / X / V", "Copy / Cut / Paste"),
                    ("Ctrl+A", "Select all"),
                    ("Ctrl+1", "Properties"),
                    ("Ctrl+9", "Command line"),
                    ("Ctrl+0", "Clean screen"),
                    ("Space / Enter", "Repeat last command or accept"),
                    ("Esc", "Cancel"),
                    ("Shift (held)", "Temporary ortho override"),
                    ("Shift+Right click", "Object snap menu"),
                    ("Wheel / MMB drag", "Zoom / pan"),
                    ("Shift+MMB drag", "Orbit (3D)"),
                ];
                egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                    egui::Grid::new("help-keys").num_columns(2).spacing([20.0, 5.0]).show(ui, |ui| {
                        for (k, d) in keys {
                            ui.label(RichText::new(k).monospace().color(t.accent2));
                            ui.label(d);
                            ui.end_row();
                        }
                    });
                });
            }
            _ => {
                ui.label("Type a command name or alias (for example L for LINE or TR for TRIM) and press Enter or Space.");
                ui.add_space(4.0);
                ui.label("Enter points as 10,20 (absolute), @10,5 (relative) or @50<30 (polar). Type a distance with the cursor in the desired direction for direct distance entry.");
                ui.add_space(4.0);
                ui.label("Prompts show options in square brackets; type the capitalised letters or click an option. Type END, MID, CEN ... for a one-off object snap, FROM for an offset base point.");
                ui.add_space(4.0);
                ui.label("Type a system variable such as OSMODE to view or change it.");
            }
        }
        self.ok_cancel(ui, None, Some("Close"), &[])
    }

    pub(super) fn ui_calc(&mut self, ui: &mut egui::Ui, s: &mut CalcState) -> Out {
        let t = self.t;
        egui::ScrollArea::vertical().max_height(180.0).stick_to_bottom(true).show(ui, |ui| {
            if s.lines.is_empty() {
                ui.label(RichText::new("Type an expression and press Enter.").color(t.text3));
            }
            for (inp, res, err) in &s.lines {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(inp).monospace().color(t.text2));
                    if *err {
                        ui.label(RichText::new(res).color(t.err));
                    } else {
                        ui.label(RichText::new("=").color(t.text3));
                        ui.label(RichText::new(res).strong());
                    }
                });
            }
        });
        let mut run = false;
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut s.input).hint_text("e.g. 2*(3+4)/sqrt(2)").desired_width(340.0).font(egui::FontId::monospace(12.0)));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                run = true;
                r.request_focus();
            }
            if self.btn(ui, "=", false).clicked() {
                run = true;
            }
        });
        if run && !s.input.trim().is_empty() {
            match eval_expr(&s.input) {
                Ok(v) => {
                    let txt = format!("{}", (v * 1e12).round() / 1e12);
                    s.lines.push((s.input.trim().to_string(), txt, false));
                    s.last = Some(v);
                }
                Err(e) => s.lines.push((s.input.trim().to_string(), format!("→ {e}"), true)),
            }
            s.input.clear();
        }
        self.note(ui, "Operators + − × ÷ ^ %, functions sin cos tan (degrees), sqrt, abs, ln, log, exp, and the constant pi.");
        let o = self.ok_cancel(ui, Some("Copy result"), Some("Close"), &[]);
        if o == Out::Ok {
            if let Some(v) = s.last {
                let txt = format!("{}", (v * 1e12).round() / 1e12);
                self.copy_text(&txt);
            }
        }
        o
    }

    /// The AutoLISP console: a code box, Run, Load, and the command history it writes to.
    pub(super) fn ui_vlisp(&mut self, ui: &mut egui::Ui, s: &mut VLispState) -> Out {
        let t = self.t;
        self.note(ui, "Write AutoLISP and press Run (Ctrl+Enter). Results appear below and in the command line history.");
        let edit = ui.add(egui::TextEdit::multiline(&mut s.src).code_editor().desired_rows(9).desired_width(f32::INFINITY).hint_text("(defun c:hello () (princ \"Hello\") (princ))"));
        let mut run = ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter)) && edit.has_focus();
        let mut out = Out::Open;
        ui.horizontal(|ui| {
            if self.btn(ui, "Run", true).clicked() {
                run = true;
            }
            if self.btn(ui, "Load file...", false).clicked() {
                self.run_command("APPLOAD", cadforge_engine::runtime::Args::none());
            }
            if self.btn(ui, "Clear", false).clicked() {
                s.src.clear();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.btn(ui, "Close", false).clicked() {
                    out = Out::Cancel;
                }
            });
        });
        if run && !s.src.trim().is_empty() {
            let src = s.src.clone();
            self.run_command("_LISP", cadforge_engine::runtime::Args::none().with("source", cadforge_engine::runtime::ArgVal::Str(src)).with("echo", cadforge_engine::runtime::ArgVal::Bool(true)));
        }
        let lines: Vec<(String, cadforge_engine::app::LogKind)> = self.h.borrow().history.iter().rev().take(14).map(|l| (l.text.clone(), l.kind)).collect::<Vec<_>>().into_iter().rev().collect();
        egui::ScrollArea::vertical().max_height(170.0).stick_to_bottom(true).auto_shrink([false, true]).show(ui, |ui| {
            for (text, kind) in &lines {
                use cadforge_engine::app::LogKind;
                let colour = match kind {
                    LogKind::Error => t.err,
                    LogKind::Input | LogKind::Cmd => t.text2,
                    _ => t.text,
                };
                ui.label(RichText::new(text).monospace().size(12.0).color(colour));
            }
        });
        out
    }

    pub(super) fn ui_purge(&mut self, ui: &mut egui::Ui, s: &mut PurgeState) -> Out {
        self.note(ui, "Unreferenced named objects that can be removed:");
        self.group(ui, &format!("Blocks ({})", s.blocks.len()), |ui| {
            if s.blocks.is_empty() {
                self.note(ui, "Nothing to purge.");
            }
            for b in &s.blocks {
                ui.label(RichText::new(b).monospace());
            }
        });
        self.group(ui, &format!("Layers ({})", s.layers.len()), |ui| {
            if s.layers.is_empty() {
                self.note(ui, "Nothing to purge.");
            }
            for b in &s.layers {
                ui.label(RichText::new(b).monospace());
            }
        });
        let has = !s.blocks.is_empty() || !s.layers.is_empty();
        self.ok_cancel(ui, if has { Some("Purge All") } else { None }, Some("Close"), &[])
    }

    pub(super) fn ui_layerstates(&mut self, ui: &mut egui::Ui, s: &mut LayerStatesState) -> Out {
        let t = self.t;
        let states: Vec<(String, usize)> = self.h.borrow().doc().and_then(|d| d.settings.layer_states.as_ref().map(|m| m.iter().map(|(k, v)| (k.clone(), v.len())).collect())).unwrap_or_default();
        if s.sel.is_none() {
            s.sel = states.first().map(|x| x.0.clone());
        }
        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
            for (name, n) in &states {
                let sel = s.sel.as_deref() == Some(name.as_str());
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), egui::Sense::click());
                if sel {
                    ui.painter().rect_filled(r, 2.0, t.accent_soft);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 2.0, t.hover);
                }
                ui.painter().text(r.left_center() + vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(12.5), t.text);
                ui.painter().text(r.right_center() - vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, format!("{n} layers"), egui::FontId::proportional(11.5), t.text3);
                if resp.clicked() {
                    s.sel = Some(name.clone());
                }
            }
        });
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut s.name).hint_text("New layer state name").desired_width(200.0));
            if self.btn(ui, "Save", false).clicked() && !s.name.trim().is_empty() {
                let n = s.name.trim().to_string();
                self.edit_doc("Layer state", |d| {
                    d.will_change();
                    let layers = d.layers.clone();
                    d.settings.layer_states.get_or_insert_with(Default::default).insert(n.clone(), layers);
                    d.touch();
                });
                s.sel = Some(s.name.trim().to_string());
                s.name.clear();
            }
        });
        let o = self.ok_cancel(ui, Some("Restore"), Some("Cancel"), &["Delete"]);
        if o == Out::Btn(0) {
            if let Some(sel) = s.sel.clone() {
                self.edit_doc("Delete layer state", |d| {
                    d.will_change();
                    if let Some(m) = d.settings.layer_states.as_mut() {
                        m.shift_remove(&sel);
                    }
                    d.touch();
                });
                s.sel = None;
            }
            return Out::Open;
        }
        o
    }

    pub(super) fn ui_aliasedit(&mut self, ui: &mut egui::Ui, s: &mut AliasState) -> Out {
        let t = self.t;
        self.note(ui, "Aliases you add override the defaults and are remembered.");
        ui.horizontal(|ui| {
            ui.label("Find");
            ui.add(egui::TextEdit::singleline(&mut s.filter).hint_text("Filter").desired_width(160.0));
        });
        let f = s.filter.trim().to_uppercase();
        let rows: Vec<(String, String, bool)> = {
            let a = self.h.borrow();
            let mut map: std::collections::BTreeMap<String, (String, bool)> = Default::default();
            for (al, c) in a.registry.all_aliases() {
                map.insert(al, (c, false));
            }
            for (al, c) in a.registry.user_aliases.borrow().iter() {
                map.insert(al.clone(), (c.clone(), true));
            }
            map.into_iter().filter(|(k, (c, _))| f.is_empty() || k.contains(&f) || c.contains(&f)).map(|(k, (c, u))| (k, c, u)).collect()
        };
        let mut remove: Option<String> = None;
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (al, c, user) in &rows {
                ui.horizontal(|ui| {
                    ui.add_sized([90.0, 18.0], egui::Label::new(RichText::new(al).monospace().color(if *user { t.accent2 } else { t.text })));
                    ui.add_sized([180.0, 18.0], egui::Label::new(RichText::new(c).monospace().color(t.text2)));
                    if *user && ui.small_button("Remove").clicked() {
                        remove = Some(al.clone());
                    }
                });
            }
        });
        if let Some(a) = remove {
            self.h.borrow().registry.user_aliases.borrow_mut().remove(&a);
            self.save_user_aliases();
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut s.new_alias).hint_text("Alias").desired_width(70.0));
            let names: Vec<String> = self.h.borrow().registry.names().into_iter().filter(|n| !n.starts_with('_') && !n.starts_with("OSNAP_")).collect();
            if s.new_cmd.is_empty() {
                s.new_cmd = "LINE".into();
            }
            egui::ComboBox::from_id_salt("alias-cmd").selected_text(&s.new_cmd).width(160.0).height(260.0).show_ui(ui, |ui| {
                for n in &names {
                    ui.selectable_value(&mut s.new_cmd, n.clone(), n);
                }
            });
            if self.btn(ui, "Add", false).clicked() {
                let a = s.new_alias.trim().to_uppercase();
                let ok = !a.is_empty() && a.len() <= 12 && a.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
                if !ok {
                    self.toast("An alias is 1 to 12 letters or digits.");
                } else if self.h.borrow().registry.has_command(&a) {
                    self.toast(&format!("{a} is already a command name."));
                } else {
                    self.h.borrow().registry.user_aliases.borrow_mut().insert(a, s.new_cmd.clone());
                    self.save_user_aliases();
                    s.new_alias.clear();
                }
            }
        });
        self.ok_cancel(ui, None, Some("Close"), &[])
    }

    pub fn save_user_aliases(&self) {
        let list: Vec<(String, String)> = self.h.borrow().registry.user_aliases.borrow().iter().map(|(a, c)| (a.clone(), c.clone())).collect();
        crate::platform::store_set("aliases", &serde_json::to_string(&list).unwrap_or_default());
    }

    pub fn load_user_aliases(&mut self) {
        if let Some(s) = crate::platform::store_get("aliases") {
            if let Ok(list) = serde_json::from_str::<Vec<(String, String)>>(&s) {
                let a = self.h.borrow();
                for (al, c) in list {
                    if a.registry.has_command(&c) {
                        a.registry.user_aliases.borrow_mut().insert(al.to_uppercase(), c);
                    }
                }
            }
        }
    }
}

#[derive(Default)]
pub struct HelpState {
    pub tab: usize,
    pub query: String,
}

#[derive(Default)]
pub struct VLispState {
    pub src: String,
}

#[derive(Default)]
pub struct CalcState {
    pub input: String,
    pub lines: Vec<(String, String, bool)>,
    pub last: Option<f64>,
}

pub struct PurgeState {
    pub blocks: Vec<String>,
    pub layers: Vec<String>,
}

impl PurgeState {
    pub fn new(app: &CadForgeApp) -> PurgeState {
        let a = app.h.borrow();
        let Some(doc) = a.doc() else { return PurgeState { blocks: vec![], layers: vec![] } };
        let mut used: BTreeSet<String> = BTreeSet::new();
        fn walk(list: &[cadforge_core::entities::Entity], doc: &cadforge_core::drawing::Drawing, used: &mut BTreeSet<String>) {
            for e in list {
                if let cadforge_core::entities::Kind::Insert(i) = &e.kind {
                    if used.insert(i.name.clone()) {
                        if let Some(b) = doc.blocks.get(&i.name) {
                            walk(&b.entities, doc, used);
                        }
                    }
                }
            }
        }
        walk(&doc.model.entities, doc, &mut used);
        for l in &doc.layouts {
            walk(&l.entities, doc, &mut used);
        }
        let blocks = doc.blocks.keys().filter(|n| !used.contains(*n)).cloned().collect();
        let layers = doc.layers.iter().filter(|l| l.name != "0" && l.name != doc.cur.layer && !doc.layer_usage(&l.name)).map(|l| l.name.clone()).collect();
        PurgeState { blocks, layers }
    }
    pub fn apply(&self, app: &mut CadForgeApp) {
        app.run_command("PURGE", cadforge_engine::runtime::Args::none());
    }
}

#[derive(Default)]
pub struct LayerStatesState {
    pub sel: Option<String>,
    pub name: String,
}

impl LayerStatesState {
    pub fn restore(&self, app: &mut CadForgeApp) {
        let Some(sel) = self.sel.clone() else { return };
        app.edit_doc("Restore layer state", |d| {
            d.will_change();
            let st = d.settings.layer_states.as_ref().and_then(|m| m.get(&sel)).cloned().unwrap_or_default();
            for s in st {
                if let Some(l) = d.layer_mut(&s.name) {
                    *l = s;
                }
            }
            d.touch();
        });
    }
}

#[derive(Default)]
pub struct AliasState {
    pub filter: String,
    pub new_alias: String,
    pub new_cmd: String,
}

// ---------------------------------------------------------------- QuickCalc expression evaluator

pub fn eval_expr(src: &str) -> Result<f64, String> {
    let t: Vec<char> = src.to_lowercase().chars().filter(|c| !c.is_whitespace()).map(|c| match c {
        '×' => '*',
        '÷' => '/',
        c => c,
    }).collect();
    struct P {
        t: Vec<char>,
        i: usize,
    }
    impl P {
        fn peek(&self) -> Option<char> {
            self.t.get(self.i).copied()
        }
        fn expr(&mut self) -> Result<f64, String> {
            let mut v = self.term()?;
            while let Some(op @ ('+' | '-')) = self.peek() {
                self.i += 1;
                let r = self.term()?;
                v = if op == '+' { v + r } else { v - r };
            }
            Ok(v)
        }
        fn term(&mut self) -> Result<f64, String> {
            let mut v = self.pow()?;
            while let Some(op @ ('*' | '/' | '%')) = self.peek() {
                self.i += 1;
                let r = self.pow()?;
                v = match op {
                    '*' => v * r,
                    '/' => v / r,
                    _ => v % r,
                };
            }
            Ok(v)
        }
        fn pow(&mut self) -> Result<f64, String> {
            let b = self.unary()?;
            if self.peek() == Some('^') {
                self.i += 1;
                let e = self.pow()?;
                return Ok(b.powf(e));
            }
            Ok(b)
        }
        fn unary(&mut self) -> Result<f64, String> {
            match self.peek() {
                Some('-') => {
                    self.i += 1;
                    Ok(-self.unary()?)
                }
                Some('+') => {
                    self.i += 1;
                    self.unary()
                }
                _ => self.atom(),
            }
        }
        fn atom(&mut self) -> Result<f64, String> {
            if self.peek() == Some('(') {
                self.i += 1;
                let v = self.expr()?;
                if self.peek() != Some(')') {
                    return Err("Missing )".into());
                }
                self.i += 1;
                return Ok(v);
            }
            let start = self.i;
            if self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                    self.i += 1;
                }
                if self.peek() == Some('e') && self.t.get(self.i + 1).is_some_and(|c| c.is_ascii_digit() || *c == '-' || *c == '+') {
                    self.i += 2;
                    while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                        self.i += 1;
                    }
                }
                let s: String = self.t[start..self.i].iter().collect();
                return s.parse().map_err(|_| "Syntax error".to_string());
            }
            while self.peek().is_some_and(|c| c.is_ascii_alphanumeric()) {
                self.i += 1;
            }
            let w: String = self.t[start..self.i].iter().collect();
            if w.is_empty() {
                return Err("Syntax error".into());
            }
            match w.as_str() {
                "pi" => return Ok(std::f64::consts::PI),
                "e" => return Ok(std::f64::consts::E),
                _ => {}
            }
            if self.peek() == Some('(') {
                self.i += 1;
                let v = self.expr()?;
                if self.peek() != Some(')') {
                    return Err("Missing )".into());
                }
                self.i += 1;
                let d2r = std::f64::consts::PI / 180.0;
                return Ok(match w.as_str() {
                    "sin" => (v * d2r).sin(),
                    "cos" => (v * d2r).cos(),
                    "tan" => (v * d2r).tan(),
                    "asin" => v.asin() / d2r,
                    "acos" => v.acos() / d2r,
                    "atan" => v.atan() / d2r,
                    "sqrt" => v.sqrt(),
                    "abs" => v.abs(),
                    "ln" => v.ln(),
                    "log" => v.log10(),
                    "exp" => v.exp(),
                    "round" => v.round(),
                    "floor" => v.floor(),
                    "ceil" => v.ceil(),
                    "r2d" => v / d2r,
                    "d2r" => v * d2r,
                    _ => return Err(format!("Unknown name \"{w}\"")),
                });
            }
            Err(format!("Unknown name \"{w}\""))
        }
    }
    let mut p = P { t, i: 0 };
    let v = p.expr()?;
    if p.i < p.t.len() {
        return Err("Syntax error".into());
    }
    if !v.is_finite() {
        return Err("Not a finite number".into());
    }
    Ok(v)
}

#[allow(dead_code)]
fn _unused(_: Color32) {}
