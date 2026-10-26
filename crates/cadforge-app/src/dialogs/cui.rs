//! Customize User Interface (CUI, QUICKCUI, CUSTOMIZE): the user's Quick Access Toolbar buttons, keyboard shortcuts
//! and ribbon panels, and the partial customization groups that are loaded. Changes apply at once; the application
//! stores the customization when it changes.
use super::Out;
use crate::app::CadForgeApp;
use cadforge_engine::cui::UserPanel;
use cadforge_engine::runtime::Args;
use eframe::egui::{self, RichText};

#[derive(Default)]
pub struct CuiDlgState {
    pub tab: usize,
    pub new_cmd: String,
    pub new_keys: String,
    pub new_title: String,
    pub new_cmds: String,
    /// Each panel's command list being edited (comma separated).
    pub edits: Vec<String>,
}

impl CuiDlgState {
    pub fn at(tab: &str) -> CuiDlgState {
        CuiDlgState { tab: match tab { "shortcuts" => 1, "panels" => 2, "partial" => 3, _ => 0 }, ..Default::default() }
    }
}

fn commands_of(text: &str) -> Vec<String> {
    text.split([',', ' ', ';']).map(|s| s.trim().to_uppercase()).filter(|s| !s.is_empty()).collect()
}

impl CadForgeApp {
    fn command_names(&self) -> Vec<String> {
        self.h.borrow().registry.names().into_iter().filter(|n| !n.starts_with('_') && !n.starts_with("OSNAP_")).collect()
    }

    fn command_combo(&self, ui: &mut egui::Ui, id: &str, value: &mut String) {
        let names = self.command_names();
        if value.is_empty() {
            *value = "LINE".into();
        }
        egui::ComboBox::from_id_salt(id).selected_text(value.as_str()).width(150.0).height(260.0).show_ui(ui, |ui| {
            for n in &names {
                ui.selectable_value(value, n.clone(), n);
            }
        });
    }

    pub(super) fn ui_cui(&mut self, ui: &mut egui::Ui, s: &mut CuiDlgState) -> Out {
        let t = self.t;
        self.tab_strip(ui, &["Quick Access Toolbar", "Keyboard Shortcuts", "Ribbon Panels", "Partial Customizations"], &mut s.tab);
        let (main, partials) = {
            let a = self.h.borrow();
            (a.cui.main.clone(), a.cui.partials.clone())
        };
        let mut out = Out::Open;
        match s.tab {
            0 => {
                let mut act: Option<(usize, i32)> = None;
                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    if main.qat.is_empty() {
                        self.note(ui, "No buttons of your own yet: add a command below, or right-click a ribbon button.");
                    }
                    for (i, q) in main.qat.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add_sized([180.0, 18.0], egui::Label::new(RichText::new(&q.cmd).monospace().color(t.text)));
                            if ui.small_button("Up").clicked() {
                                act = Some((i, -1));
                            }
                            if ui.small_button("Down").clicked() {
                                act = Some((i, 1));
                            }
                            if ui.small_button("Remove").clicked() {
                                act = Some((i, 0));
                            }
                        });
                    }
                    for p in &partials {
                        for q in &p.cui.qat {
                            ui.label(RichText::new(format!("{}   ({})", q.cmd, p.name)).monospace().color(t.text3));
                        }
                    }
                });
                if let Some((i, d)) = act {
                    let mut a = self.h.borrow_mut();
                    let qat = &mut a.cui.main.qat;
                    match d {
                        0 => {
                            qat.remove(i);
                        }
                        -1 if i > 0 => qat.swap(i, i - 1),
                        1 if i + 1 < qat.len() => qat.swap(i, i + 1),
                        _ => {}
                    }
                    a.cui.touch();
                }
                ui.separator();
                ui.horizontal(|ui| {
                    self.command_combo(ui, "cui-qat-cmd", &mut s.new_cmd);
                    if self.btn(ui, "Add", false).clicked() {
                        let added = self.h.borrow_mut().cui.add_qat(&s.new_cmd, None);
                        if !added {
                            self.toast(&format!("{} is already on the Quick Access Toolbar.", s.new_cmd));
                        }
                    }
                });
            }
            1 => {
                let mut remove: Option<String> = None;
                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    if main.shortcuts.is_empty() {
                        self.note(ui, "No shortcuts of your own yet.");
                    }
                    for sc in &main.shortcuts {
                        ui.horizontal(|ui| {
                            ui.add_sized([130.0, 18.0], egui::Label::new(RichText::new(&sc.keys).monospace().color(t.text)));
                            ui.add_sized([150.0, 18.0], egui::Label::new(RichText::new(&sc.cmd).monospace().color(t.text2)));
                            if ui.small_button("Remove").clicked() {
                                remove = Some(sc.keys.clone());
                            }
                        });
                    }
                    for p in &partials {
                        for sc in &p.cui.shortcuts {
                            ui.label(RichText::new(format!("{:<16}{}   ({})", sc.keys, sc.cmd, p.name)).monospace().color(t.text3));
                        }
                    }
                });
                if let Some(k) = remove {
                    let mut a = self.h.borrow_mut();
                    a.cui.main.shortcuts.retain(|x| x.keys != k);
                    a.cui.touch();
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut s.new_keys).hint_text("Ctrl+Shift+L").desired_width(110.0));
                    self.command_combo(ui, "cui-key-cmd", &mut s.new_cmd);
                    if self.btn(ui, "Add", false).clicked() {
                        let r = self.h.borrow_mut().cui.set_shortcut(&s.new_keys, &s.new_cmd);
                        match r {
                            Ok(_) => s.new_keys.clear(),
                            Err(e) => self.toast(&e),
                        }
                    }
                });
                self.note(ui, "Use Ctrl or Alt with a key, or a function key; a shortcut of your own comes before the built-in one.");
            }
            2 => {
                s.edits.resize(main.panels.len(), String::new());
                let mut remove: Option<usize> = None;
                let mut save: Option<usize> = None;
                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    if main.panels.is_empty() {
                        self.note(ui, "No panels of your own yet. They appear on the Custom ribbon tab.");
                    }
                    for (i, p) in main.panels.iter().enumerate() {
                        if s.edits[i].is_empty() {
                            s.edits[i] = p.cmds.join(", ");
                        }
                        ui.horizontal(|ui| {
                            ui.add_sized([110.0, 18.0], egui::Label::new(RichText::new(&p.title).strong().color(t.text)));
                            if ui.add(egui::TextEdit::singleline(&mut s.edits[i]).desired_width(220.0)).lost_focus() {
                                save = Some(i);
                            }
                            if ui.small_button("Remove").clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    for p in &partials {
                        for pn in &p.cui.panels {
                            ui.label(RichText::new(format!("{}: {}   ({})", pn.title, pn.cmds.join(", "), p.name)).color(t.text3));
                        }
                    }
                });
                if let Some(i) = save {
                    let cmds = commands_of(&s.edits[i]);
                    let mut a = self.h.borrow_mut();
                    if a.cui.main.panels[i].cmds != cmds {
                        a.cui.main.panels[i].cmds = cmds;
                        a.cui.touch();
                    }
                }
                if let Some(i) = remove {
                    let mut a = self.h.borrow_mut();
                    a.cui.main.panels.remove(i);
                    a.cui.touch();
                    s.edits.remove(i);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut s.new_title).hint_text("Panel title").desired_width(110.0));
                    ui.add(egui::TextEdit::singleline(&mut s.new_cmds).hint_text("LINE, CIRCLE, ...").desired_width(180.0));
                    if self.btn(ui, "Add", false).clicked() {
                        let (title, cmds) = (s.new_title.trim().to_string(), commands_of(&s.new_cmds));
                        let known: Vec<String> = cmds.iter().filter(|c| self.h.borrow().registry.has_command(c)).cloned().collect();
                        if title.is_empty() || known.is_empty() {
                            self.toast("A panel needs a title and at least one command.");
                        } else {
                            let mut a = self.h.borrow_mut();
                            a.cui.main.panels.push(UserPanel { title, cmds: known });
                            a.cui.touch();
                            drop(a);
                            s.new_title.clear();
                            s.new_cmds.clear();
                        }
                    }
                });
            }
            _ => {
                let mut unload: Option<String> = None;
                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    if partials.is_empty() {
                        self.note(ui, "No partial customization groups are loaded.");
                    }
                    for p in &partials {
                        ui.horizontal(|ui| {
                            ui.add_sized([140.0, 18.0], egui::Label::new(RichText::new(&p.name).strong().color(t.text)));
                            ui.label(RichText::new(format!("{} buttons, {} shortcuts, {} panels", p.cui.qat.len(), p.cui.shortcuts.len(), p.cui.panels.len())).color(t.text2));
                            if ui.small_button("Unload").clicked() {
                                unload = Some(p.name.clone());
                            }
                        });
                    }
                });
                if let Some(n) = unload {
                    self.h.borrow_mut().cui.unload(&n);
                }
            }
        }
        match self.ok_cancel(ui, None, Some("Close"), &["Load...", "Import...", "Export..."]) {
            Out::Btn(0) => self.open_files(&ui.ctx().clone(), "cuiload"),
            Out::Btn(1) => self.open_files(&ui.ctx().clone(), "cuiimport"),
            Out::Btn(_) => {
                self.run_command("CUIEXPORT", Args::none());
                self.h.pump();
            }
            o => out = o,
        }
        out
    }
}
