//! Keyboard handling: function keys, shortcuts and the command line (typing, history, suggestions).
use crate::app::CadForgeApp;
use cadforge_engine::runtime::{Args, PromptKind};
use eframe::egui::{self, Event, Key};

impl CadForgeApp {
    pub fn handle_keys(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        let typing = ctx.egui_wants_keyboard_input();
        let modal = self.dialog.is_some();
        // Shift held: temporary ortho override while a command runs
        let shift = ctx.input(|i| i.modifiers.shift);
        if shift != self.shift_prev {
            self.shift_prev = shift;
            let busy = self.h.borrow().busy();
            let mut a = self.h.borrow_mut();
            if shift && busy {
                a.rt.shift_ortho = true;
                a.emit(cadforge_engine::app::Event::Overlay);
            } else if !shift && a.rt.shift_ortho {
                a.rt.shift_ortho = false;
                a.emit(cadforge_engine::app::Event::Overlay);
            }
        }
        for ev in events {
            match ev {
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    repeat,
                    ..
                } => {
                    let ctrl = modifiers.command || modifiers.ctrl;
                    if modal {
                        if key == Key::Escape {
                            self.dialog_escape();
                        }
                        continue;
                    }
                    // function keys always work
                    if !ctrl {
                        let handled = match key {
                            Key::F1 => {
                                self.run_command("HELP", Args::none());
                                true
                            }
                            Key::F2 => {
                                self.ui.text_window = !self.ui.text_window;
                                true
                            }
                            Key::F3 => {
                                self.toggle_osnap();
                                true
                            }
                            Key::F7 => {
                                self.toggle_sv("GRIDMODE");
                                true
                            }
                            Key::F8 => {
                                self.toggle_ortho();
                                true
                            }
                            Key::F9 => {
                                self.toggle_sv("SNAPMODE");
                                true
                            }
                            Key::F10 => {
                                self.toggle_polar();
                                true
                            }
                            Key::F11 => {
                                self.toggle_sv("OTRACKMODE");
                                true
                            }
                            Key::F12 => {
                                self.toggle_dyn();
                                true
                            }
                            _ => false,
                        };
                        if handled {
                            continue;
                        }
                    }
                    if key == Key::Escape {
                        if typing && self.ui.search_focus {
                            continue;
                        }
                        self.escape();
                        continue;
                    }
                    if ctrl {
                        let shiftk = modifiers.shift;
                        let cmd = match key {
                            Key::N => Some("NEW"),
                            Key::O => Some("OPEN"),
                            Key::S => Some(if shiftk { "SAVEAS" } else { "QSAVE" }),
                            Key::P => Some(if shiftk { "QUICKPROPERTIES" } else { "PLOT" }),
                            Key::Z => Some("U"),
                            Key::Y => Some("REDO"),
                            Key::A => Some("SELECTALL"),
                            Key::C => Some("COPYCLIP"),
                            Key::X => Some("CUTCLIP"),
                            Key::V => Some("PASTECLIP"),
                            Key::Num1 => Some("PROPERTIES"),
                            Key::Num2 | Key::Num3 => Some("TOOLPALETTES"),
                            Key::Num0 => Some("CLEANSCREENON"),
                            Key::Num9 => Some("COMMANDLINE"),
                            _ => None,
                        };
                        if let Some(c) = cmd {
                            // normal text editing wins while typing in a field (or when the command line has text)
                            if typing
                                && matches!(
                                    key,
                                    Key::A | Key::C | Key::X | Key::V | Key::Z | Key::Y
                                )
                            {
                                continue;
                            }
                            if !typing
                                && matches!(key, Key::C | Key::X | Key::V | Key::A)
                                && !self.ui.cmd.text.is_empty()
                            {
                                continue;
                            }
                            if c == "CLEANSCREENON" {
                                self.ui.clean = !self.ui.clean;
                            } else {
                                self.run_command(c, Args::none());
                            }
                        }
                        continue;
                    }
                    if typing || modifiers.alt {
                        continue;
                    }
                    self.cmd_key(ctx, key, repeat);
                }
                Event::Text(s) => {
                    if typing || modal || s.chars().all(|c| c.is_control()) {
                        continue;
                    }
                    self.cmd_type(&s);
                }
                Event::Paste(s) => {
                    if !typing && !modal {
                        let one: String = s.lines().next().unwrap_or("").to_string();
                        self.cmd_type(&one);
                    }
                }
                _ => {}
            }
        }
    }

    fn cmd_key(&mut self, ctx: &egui::Context, key: Key, _repeat: bool) {
        let (busy, kind, has_opts, recent_input) = {
            let a = self.h.borrow();
            (
                a.busy(),
                a.pending().map(|p| p.kind),
                a.pending().is_some_and(|p| !p.spec.options.is_empty()),
                a.rt.recent_input.clone(),
            )
        };
        match key {
            Key::Enter => {
                if let Some(i) = self.ui.cmd.sug_idx {
                    if let Some(s) = self.ui.cmd.sugs.get(i).cloned() {
                        self.ui.cmd.text = s.clone();
                        self.ui.cmd.caret = s.chars().count();
                    }
                }
                self.submit_cmd();
            }
            Key::Space => {
                let c = &self.ui.cmd;
                if kind != Some(PromptKind::String) && !c.text.trim().contains(char::is_whitespace)
                {
                    if let Some(i) = c.sug_idx {
                        if let Some(s) = c.sugs.get(i).cloned() {
                            self.ui.cmd.text = s;
                        }
                    }
                    self.submit_cmd();
                } else {
                    self.cmd_type(" ");
                }
            }
            Key::Backspace => {
                let c = &mut self.ui.cmd;
                if c.caret > 0 {
                    let mut chars: Vec<char> = c.text.chars().collect();
                    chars.remove(c.caret - 1);
                    c.text = chars.into_iter().collect();
                    c.caret -= 1;
                    self.cmd_changed();
                }
            }
            Key::Delete => {
                let sel = self.h.borrow().editor().is_some_and(|e| !e.sel.is_empty());
                if self.ui.cmd.text.is_empty() && !busy && sel {
                    self.run_command("ERASE", Args::none());
                } else {
                    let c = &mut self.ui.cmd;
                    let mut chars: Vec<char> = c.text.chars().collect();
                    if c.caret < chars.len() {
                        chars.remove(c.caret);
                        c.text = chars.into_iter().collect();
                        self.cmd_changed();
                    }
                }
            }
            Key::ArrowLeft => self.ui.cmd.caret = self.ui.cmd.caret.saturating_sub(1),
            Key::ArrowRight => {
                self.ui.cmd.caret = (self.ui.cmd.caret + 1).min(self.ui.cmd.text.chars().count())
            }
            Key::Home => self.ui.cmd.caret = 0,
            Key::End => self.ui.cmd.caret = self.ui.cmd.text.chars().count(),
            Key::ArrowUp | Key::ArrowDown => {
                let up = key == Key::ArrowUp;
                let c = &mut self.ui.cmd;
                if !c.sugs.is_empty() {
                    let n = c.sugs.len();
                    c.sug_idx = Some(match (c.sug_idx, up) {
                        (None, true) => n - 1,
                        (None, false) => 0,
                        (Some(i), false) => (i + 1) % n,
                        (Some(0), true) => n - 1,
                        (Some(i), true) => i - 1,
                    });
                    return;
                }
                if !up && has_opts && c.text.is_empty() {
                    self.option_menu(ctx);
                    return;
                }
                let src: Vec<String> = if busy {
                    recent_input
                } else {
                    self.ui.cmd.past.clone()
                };
                let c = &mut self.ui.cmd;
                let cur = c.recall.map_or(-1, |i| i as i64);
                let next = (cur + if up { 1 } else { -1 }).clamp(-1, src.len() as i64 - 1);
                c.recall = if next >= 0 { Some(next as usize) } else { None };
                c.text = if next >= 0 {
                    src[next as usize].clone()
                } else {
                    String::new()
                };
                c.caret = c.text.chars().count();
            }
            Key::Tab => {
                if !self.ui.cmd.sugs.is_empty() {
                    let i = self.ui.cmd.sug_idx.unwrap_or(0);
                    if let Some(s) = self.ui.cmd.sugs.get(i).cloned() {
                        self.ui.cmd.caret = s.chars().count();
                        self.ui.cmd.text = s;
                        self.ui.cmd.sugs.clear();
                        self.ui.cmd.sug_idx = None;
                    }
                }
            }
            _ => {}
        }
        if matches!(key, Key::Delete) && !busy {
            // Delete with an empty command line erases the selection (handled above)
        }
        self.h
            .borrow_mut()
            .emit(cadforge_engine::app::Event::Overlay);
    }

    fn option_menu(&mut self, ctx: &egui::Context) {
        let opts: Vec<String> = self
            .h
            .borrow()
            .pending()
            .map(|p| p.spec.options.clone())
            .unwrap_or_default();
        let items: Vec<crate::menu::MItem> = opts
            .iter()
            .map(|o| {
                let label = cadforge_engine::parse::option_name(o);
                let o2 = o.clone();
                crate::menu::MItem::item(&label).on(move |a| a.h.keyword(&o2))
            })
            .collect();
        if let Some(r) = self.last_cmd_row {
            self.dropup(ctx, r, items, 160.0);
        }
    }

    pub fn cmd_type(&mut self, s: &str) {
        let (pending, pointer_input) = {
            let a = self.h.borrow();
            (a.pending().is_some(), a.sysvars.pointer_input())
        };
        let c = &mut self.ui.cmd;
        if c.text.is_empty() {
            c.dynamic = pointer_input && pending;
        }
        let mut chars: Vec<char> = c.text.chars().collect();
        for (i, ch) in s.chars().filter(|c| !c.is_control()).enumerate() {
            chars.insert(c.caret + i, ch);
        }
        c.caret += s.chars().filter(|c| !c.is_control()).count();
        c.text = chars.into_iter().collect();
        self.cmd_changed();
    }

    /// Recompute the suggestion list after the text changed.
    fn cmd_changed(&mut self) {
        let v = self.ui.cmd.text.trim().to_string();
        self.ui.cmd.recall = None;
        self.ui.cmd.sug_idx = None;
        let (busy, auto) = {
            let a = self.h.borrow();
            (a.busy(), a.sysvars.int("AUTOCOMPLETE") != 0)
        };
        let starts_numeric = v
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || "@#.,<-".contains(c));
        if busy || v.is_empty() || !auto || starts_numeric {
            self.ui.cmd.sugs.clear();
            return;
        }
        let a = self.h.borrow();
        let mut cmds: Vec<String> = a
            .registry
            .suggest(&v, true, &[], 14)
            .into_iter()
            .filter(|n| !n.starts_with('_') && !n.starts_with("OSNAP_"))
            .collect();
        let vars: Vec<String> = if v.len() >= 3 {
            a.sysvars
                .names()
                .into_iter()
                .filter(|n| n.starts_with(&v.to_uppercase()) && !cmds.contains(n))
                .take(4)
                .collect()
        } else {
            vec![]
        };
        cmds.truncate(10);
        cmds.extend(vars);
        drop(a);
        self.ui.cmd.sugs = cmds;
    }

    pub fn submit_cmd(&mut self) {
        let text = std::mem::take(&mut self.ui.cmd.text);
        let dynamic = self.ui.cmd.dynamic;
        self.ui.cmd.caret = 0;
        self.ui.cmd.dynamic = false;
        self.ui.cmd.recall = None;
        self.ui.cmd.sugs.clear();
        self.ui.cmd.sug_idx = None;
        let busy = self.h.borrow().busy();
        if !busy && !text.trim().is_empty() {
            let t = text.trim().to_string();
            let past = &mut self.ui.cmd.past;
            if past.first() != Some(&t) {
                past.insert(0, t);
            }
            past.truncate(60);
        }
        self.h.text(&text, dynamic);
        self.after_input();
    }

    pub fn after_input(&mut self) {
        // pick up a command that asked for a dialog etc. on the next frame
        self.h
            .borrow_mut()
            .emit(cadforge_engine::app::Event::Overlay);
    }
}
