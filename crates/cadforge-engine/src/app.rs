//! Application state (port of `src/engine/app.js`): open drawings (editors), preferences, command history, the
//! runtime, and [`Handle`] — the cheap-clone reference through which the UI, commands and tests drive everything.
use crate::editor::Editor;
use crate::memory::Memory;
use crate::registry::Registry;
use crate::runtime::*;
use crate::sysvars::Sysvars;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::Entity;
use cadforge_core::geom::Pt;
use std::cell::{Ref, RefCell, RefMut};
use std::rc::{Rc, Weak};
use std::task::{Context, Poll, Waker};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogKind {
    Info,
    Cmd,
    Prompt,
    Input,
    Error,
}

impl From<&str> for LogKind {
    fn from(s: &str) -> LogKind {
        match s {
            "cmd" => LogKind::Cmd,
            "prompt" => LogKind::Prompt,
            "input" => LogKind::Input,
            "error" => LogKind::Error,
            _ => LogKind::Info,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub text: String,
    pub kind: LogKind,
}

/// Notifications for the UI (drained with [`App::take_events`]).
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Scene or selection changed: repaint everything.
    Change,
    /// Only the overlay (cursor, rubber band) changed.
    Overlay,
    Log,
    Prompt,
    Drawings,
    Active,
    CommandEnd(String),
    Clipboard,
    ShowHistory,
    Regen,
    HatchSession,
    /// Show / hide / toggle (`None`) a UI palette by name.
    Palette {
        name: String,
        show: Option<bool>,
    },
    /// A command delegates to the host UI (open/save/plot/settings dialogs...).
    UiAction {
        name: String,
        args: Args,
    },
    /// Write these entities to a new drawing file (WBLOCK).
    Export(Vec<Entity>),
    /// Export solids as `obj` / `stl`.
    ExportMesh(String),
    /// Hand a generated file (CSV extracts, images...) to the host: download in the browser, write next to the drawing natively.
    SaveFile {
        name: String,
        data: Vec<u8>,
        mime: String,
    },
    /// A command asked for a modal dialog (see [`App::pending`]).
    Dialog,
}

#[derive(Clone, Debug, Default)]
pub struct Clipboard {
    pub entities: Vec<Entity>,
    pub base: Pt,
}

/// An active HATCH / HATCHEDIT session (drives the contextual hatch editor in the UI).
#[derive(Clone)]
pub struct HatchSession {
    pub settings: Rc<RefCell<crate::memory::HatchMemory>>,
    pub editing: Option<u64>,
    pub count: Rc<std::cell::Cell<usize>>,
}

/// What the host UI can do for commands.
#[derive(Clone, Copy, Debug, Default)]
pub struct UiCaps {
    pub dialogs: bool,
}

pub struct App {
    pub editors: Vec<Editor>,
    pub active: Option<usize>,
    pub sysvars: Sysvars,
    pub registry: Rc<Registry>,
    pub history: Vec<LogLine>,
    pub clipboard: Option<Clipboard>,
    pub mem: Memory,
    pub rt: Runtime,
    /// Monotonic clock in milliseconds, set by the host.
    pub now: f64,
    pub ui: UiCaps,
    /// Incremented on every `Change` event; hosts repaint when it moves.
    pub rev: u64,
    pub recent: Vec<String>,
    pub hatch_session: Option<HatchSession>,
    events: Vec<Event>,
    counter: u32,
    next_editor_id: u64,
    me: Weak<RefCell<App>>,
    deferred_start: Option<(String, Args)>,
    script: std::collections::VecDeque<String>,
    /// Lines a running Lisp `(command ...)` feeds to the command it started; `None` is PAUSE (wait for the user).
    pub(crate) lisp_input: std::collections::VecDeque<Option<String>>,
    /// Counts prompts; a Lisp PAUSE waits for the prompt it was reached at to be answered.
    pub(crate) prompt_seq: u64,
    /// The Action Recorder: the command line input of the commands run since ACTRECORD.
    pub recording: Option<Vec<String>>,
    pub(crate) lisp_pause: Option<u64>,
    /// The interpreter shared by every Lisp command of this session.
    pub lisp: Rc<crate::lisp::Interp>,
    /// Autosave bookkeeping (see [`crate::autosave`]).
    pub autosave: crate::autosave::AutoSaveState,
}

impl App {
    pub fn new(registry: Rc<Registry>) -> App {
        App {
            editors: vec![],
            active: None,
            sysvars: Sysvars::default(),
            registry,
            history: vec![],
            clipboard: None,
            mem: Memory::default(),
            rt: Runtime::default(),
            now: 0.0,
            ui: UiCaps::default(),
            rev: 0,
            recent: vec![],
            hatch_session: None,
            events: vec![],
            counter: 0,
            next_editor_id: 1,
            me: Weak::new(),
            deferred_start: None,
            script: Default::default(),
            lisp_input: Default::default(),
            prompt_seq: 0,
            recording: None,
            lisp_pause: None,
            lisp: Rc::new(crate::lisp::Interp::new()),
            autosave: Default::default(),
        }
    }

    pub fn handle(&self) -> Handle {
        Handle(self.me.upgrade().expect("App is owned by a Handle"))
    }

    /// The current user coordinate system (world when there is no drawing).
    pub fn ucs(&self) -> cadforge_core::geom::Ucs {
        self.doc().map(|d| d.ucs).unwrap_or_default()
    }

    pub fn editor(&self) -> Option<&Editor> {
        self.active.and_then(|i| self.editors.get(i))
    }
    pub fn editor_mut(&mut self) -> Option<&mut Editor> {
        let i = self.active?;
        self.editors.get_mut(i)
    }
    pub fn doc(&self) -> Option<&Drawing> {
        self.editor().map(|e| &e.doc)
    }

    pub fn getvar(&self, name: &str) -> Result<crate::sysvars::Value, String> {
        self.sysvars.get(name, self.doc())
    }

    pub fn setvar(
        &mut self,
        name: &str,
        v: impl Into<crate::sysvars::Value>,
    ) -> Result<(), String> {
        let i = self.active.unwrap_or(0);
        let doc = self.editors.get_mut(i).map(|e| &mut e.doc);
        self.sysvars.set(name, v, doc)
    }

    pub fn emit(&mut self, e: Event) {
        if e == Event::Change {
            self.rev += 1;
        }
        if self.events.len() > 5000 {
            self.events.drain(..2500);
        }
        self.events.push(e);
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub fn log(&mut self, text: &str, kind: &str) {
        self.history.push(LogLine {
            text: text.to_string(),
            kind: kind.into(),
        });
        if self.history.len() > 2000 {
            let n = self.history.len() - 2000;
            self.history.drain(..n);
        }
        self.emit(Event::Log);
    }

    // ------------------------------------------------------------ drawings

    pub fn new_drawing(&mut self, name: Option<&str>, imperial: bool) -> u64 {
        self.counter += 1;
        let mut doc = Drawing::new(
            &name
                .map(str::to_string)
                .unwrap_or_else(|| format!("Drawing{}", self.counter)),
        );
        if imperial {
            let st = &mut doc.settings;
            st.units.ins_units = 1;
            st.units.precision = 4;
            st.text_height = 0.2;
            let d = st
                .dim_styles
                .get_mut("Standard")
                .expect("Standard dim style");
            d.txt = 0.18;
            d.asz = 0.18;
            d.exo = 0.0625;
            d.exe = 0.18;
            d.gap = 0.09;
            d.cen = 0.09;
            d.dec = 4;
        }
        self.add_drawing(doc)
    }

    pub fn add_drawing(&mut self, mut doc: Drawing) -> u64 {
        doc.ensure_default_layouts();
        let id = self.next_editor_id;
        self.next_editor_id += 1;
        self.editors.push(Editor::new(id, doc));
        let idx = self.editors.len() - 1;
        self.set_active(idx);
        self.emit(Event::Drawings);
        id
    }

    pub fn set_active(&mut self, idx: usize) {
        if self.active == Some(idx) || idx >= self.editors.len() {
            return;
        }
        if self.active.is_some() && self.busy() {
            self.escape();
        }
        self.active = Some(idx);
        self.rt_attach();
        self.emit(Event::Active);
        self.emit(Event::Change);
    }

    pub fn close_editor(&mut self, idx: usize) {
        if idx >= self.editors.len() {
            return;
        }
        let closed = self.editors.remove(idx);
        self.autosave.editor_closed(closed.id);
        match self.active {
            Some(a) if a == idx => {
                self.active = None;
                if !self.editors.is_empty() {
                    let next = idx.saturating_sub(1).min(self.editors.len() - 1);
                    self.set_active(next);
                }
            }
            Some(a) if a > idx => self.active = Some(a - 1),
            _ => {}
        }
        self.emit(Event::Drawings);
        self.emit(Event::Change);
    }

    pub(crate) fn take_deferred_start(&mut self) -> Option<(String, Args)> {
        self.deferred_start.take()
    }

    /// Feed lines to the command line one at a time (SCRIPT), skipping `;` comments.
    pub fn queue_script(&mut self, lines: Vec<String>) {
        self.script.extend(
            lines
                .into_iter()
                .filter(|l| !l.trim_start().starts_with(';')),
        );
    }

    pub(crate) fn next_script_line(&mut self) -> Option<String> {
        self.script.pop_front()
    }

    /// Queue a command to start as soon as the current one has unwound.
    pub fn start_deferred(&mut self, name: &str, args: Args) {
        self.deferred_start = Some((name.to_string(), args));
    }
}

/// Shared handle to the [`App`]. Every input method runs the event handler and then lets any resumed command futures
/// run to their next prompt (`pump`), so one call ≙ one turn of the JS event loop.
#[derive(Clone)]
pub struct Handle(pub Rc<RefCell<App>>);

impl Handle {
    pub fn new(mut app: App) -> Handle {
        Handle(Rc::new_cyclic(|me| {
            app.me = me.clone();
            RefCell::new(app)
        }))
    }

    pub fn borrow(&self) -> Ref<'_, App> {
        self.0.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<'_, App> {
        self.0.borrow_mut()
    }

    /// Resume every queued command future until all are waiting for input again.
    pub fn pump(&self) {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        loop {
            let next = {
                let mut app = self.0.borrow_mut();
                app.rt.ready.pop_front()
            };
            let Some(id) = next else {
                // a deferred start waits until the stack has unwound
                let deferred = {
                    let mut app = self.0.borrow_mut();
                    if app.busy() {
                        None
                    } else {
                        app.take_deferred_start()
                    }
                };
                match deferred {
                    Some((name, args)) => {
                        self.0.borrow_mut().run(&name, args, false);
                        continue;
                    }
                    None => {
                        // a Lisp `(command ...)` has priority; PAUSE hands the next answer to the user
                        let holding = {
                            let mut a = self.0.borrow_mut();
                            match a.lisp_pause {
                                Some(seq) if seq == a.prompt_seq && a.busy() => true,
                                _ => {
                                    a.lisp_pause = None;
                                    false
                                }
                            }
                        };
                        if holding {
                            break;
                        }
                        let lisp_line = self.0.borrow_mut().lisp_input.pop_front();
                        match lisp_line {
                            Some(Some(l)) => {
                                self.0.borrow_mut().text(&l, false);
                                continue;
                            }
                            Some(None) => {
                                let mut a = self.0.borrow_mut();
                                a.lisp_pause = Some(a.prompt_seq);
                                break;
                            }
                            None => {}
                        }
                        // scripts feed the same input channel as the command line
                        let line = self.0.borrow_mut().next_script_line();
                        match line {
                            Some(l) => {
                                self.0.borrow_mut().text(&l, false);
                                continue;
                            }
                            None => break,
                        }
                    }
                }
            };
            let fut = self
                .0
                .borrow_mut()
                .rt
                .tasks
                .get_mut(&id)
                .and_then(|t| t.fut.take());
            let Some(mut fut) = fut else { continue };
            match fut.as_mut().poll(&mut cx) {
                Poll::Pending => {
                    if let Some(t) = self.0.borrow_mut().rt.tasks.get_mut(&id) {
                        t.fut = Some(fut);
                    }
                }
                Poll::Ready(res) => self.0.borrow_mut().end_task(id, res),
            }
        }
    }

    fn with<R>(&self, f: impl FnOnce(&mut App) -> R) -> R {
        let r = f(&mut self.0.borrow_mut());
        self.pump();
        r
    }

    /// Run a command by name/alias as if typed.
    pub fn run(&self, name: &str) {
        self.with(|a| {
            a.run(name, Args::none(), false);
        });
    }

    /// From the UI (ribbon, menus): cancels whatever is running, then starts the command.
    pub fn start(&self, name: &str, args: Args) {
        self.with(|a| {
            a.start(name, args);
        });
    }

    /// Close a drawing tab, cancelling any running command first.
    pub fn close_editor(&self, idx: usize) {
        self.with(|a| {
            if a.busy() {
                a.escape();
            }
        });
        self.with(|a| a.close_editor(idx));
    }

    pub fn mouse_move(&self, raw: Pt) {
        self.with(|a| a.mouse_move(raw));
    }
    pub fn click(&self, raw: Pt, mods: Mods) {
        self.with(|a| a.click(raw, mods));
    }
    pub fn drag(&self, a_: Pt, b: Pt, mods: Mods) {
        self.with(|a| a.drag(a_, b, mods));
    }
    pub fn double_click(&self, raw: Pt) {
        self.with(|a| a.double_click(raw));
    }
    /// Text from the command line (`dynamic` = from the dynamic-input tooltip).
    pub fn text(&self, s: &str, dynamic: bool) {
        self.with(|a| a.text(s, dynamic));
    }
    pub fn enter(&self) {
        self.with(|a| a.enter());
    }
    pub fn escape(&self) {
        self.with(|a| a.escape());
    }
    /// Answer the modal dialog a command is waiting for.
    pub fn answer_dialog(&self, reply: DialogReply) {
        self.with(|a| a.answer_dialog(reply));
    }
    pub fn keyword(&self, label: &str) {
        self.with(|a| a.keyword(label));
    }
    /// Run a list of command-line lines (SCRIPT).
    pub fn script(&self, lines: &[&str]) {
        for l in lines {
            if l.trim_start().starts_with(';') {
                continue;
            }
            self.text(l, false);
        }
    }
}
