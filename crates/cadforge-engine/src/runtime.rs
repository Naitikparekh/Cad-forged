//! Command runtime (port of `src/engine/runtime.js`): runs commands as async functions that await prompts and routes
//! pointer / keyboard input (clicks, typed text, Enter, Escape) to whatever prompt is pending.
//!
//! Commands are `async fn(Ctx, Args)`. Each prompt (`ctx.point(..).await`) registers a [`Pending`] on the task and
//! suspends; input handlers on [`App`] resolve it with [`App::deliver`], which queues the task so the executor
//! ([`crate::app::Handle::pump`]) resumes it. The runtime is single-threaded and DOM/GUI-free: tests drive it directly.
use crate::app::{App, Event};
use crate::parse::*;
use crate::select::*;
use crate::snap::*;
use crate::solid_pick::{pick_solid, solids_in_screen_rect};
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::{grips_of, Entity};
use cadforge_core::geom::*;
use indexmap::IndexSet;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

pub type TaskId = usize;
pub type CmdFuture = Pin<Box<dyn Future<Output = CmdResult<()>>>>;
pub type PreviewFn = Rc<dyn Fn(&Drawing, Pt) -> Vec<PreviewItem>>;
pub type EntFilter = Rc<dyn Fn(&Entity) -> bool>;

/// Why a command stopped early.
#[derive(Clone, Debug, PartialEq)]
pub enum CmdError {
    /// The user pressed Escape (`*Cancel*`).
    Cancel,
    /// A reportable failure.
    Msg(String),
}

impl From<String> for CmdError {
    fn from(s: String) -> CmdError {
        CmdError::Msg(s)
    }
}
impl From<&str> for CmdError {
    fn from(s: &str) -> CmdError {
        CmdError::Msg(s.to_string())
    }
}

pub type CmdResult<T> = Result<T, CmdError>;

/// Items drawn as the rubber-band preview while a prompt is pending.
#[derive(Clone, Debug)]
pub enum PreviewItem {
    Entity(Entity),
    Rect { a: Pt, b: Pt },
}

// ---------------------------------------------------------------- command arguments

#[derive(Clone, Debug, PartialEq)]
pub enum ArgVal {
    Num(f64),
    Str(String),
    Id(u64),
    Bool(bool),
    Pt(Pt),
    Grip(GripRef),
    Ids(Vec<u64>),
    Lines(Vec<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GripRef {
    pub entity: u64,
    pub id: String,
    pub pt: Pt,
    pub kind: cadforge_core::entities::GripKind,
}

/// Optional arguments passed to a command by the UI (ribbon, menus, grips...).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args(pub HashMap<&'static str, ArgVal>);

impl Args {
    pub fn none() -> Args {
        Args::default()
    }
    pub fn with(mut self, k: &'static str, v: ArgVal) -> Args {
        self.0.insert(k, v);
        self
    }
    pub fn str(&self, k: &str) -> Option<&str> {
        match self.0.get(k) {
            Some(ArgVal::Str(s)) => Some(s),
            _ => None,
        }
    }
    pub fn num(&self, k: &str) -> Option<f64> {
        match self.0.get(k) {
            Some(ArgVal::Num(n)) => Some(*n),
            _ => None,
        }
    }
    pub fn id(&self, k: &str) -> Option<u64> {
        match self.0.get(k) {
            Some(ArgVal::Id(n)) => Some(*n),
            _ => None,
        }
    }
    pub fn flag(&self, k: &str) -> bool {
        matches!(self.0.get(k), Some(ArgVal::Bool(true)))
    }
    pub fn pt(&self, k: &str) -> Option<Pt> {
        match self.0.get(k) {
            Some(ArgVal::Pt(p)) => Some(*p),
            _ => None,
        }
    }
    /// A boolean argument if it was supplied at all.
    pub fn opt_bool(&self, k: &str) -> Option<bool> {
        match self.0.get(k) {
            Some(ArgVal::Bool(b)) => Some(*b),
            _ => None,
        }
    }
    pub fn lines(&self, k: &str) -> Option<&[String]> {
        match self.0.get(k) {
            Some(ArgVal::Lines(v)) => Some(v),
            _ => None,
        }
    }
    pub fn ids(&self, k: &str) -> Option<&[u64]> {
        match self.0.get(k) {
            Some(ArgVal::Ids(v)) => Some(v),
            _ => None,
        }
    }
    pub fn grip(&self, k: &str) -> Option<&GripRef> {
        match self.0.get(k) {
            Some(ArgVal::Grip(g)) => Some(g),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------- prompts

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptKind {
    Point,
    Distance,
    Angle,
    Number,
    String,
    Keyword,
    Entity,
    Solid,
    Select,
    Dialog,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PromptDefault {
    Pt(Pt),
    Num(f64),
    Str(String),
}

/// Everything a prompt can be configured with (the union of the JS prompt-spec options).
#[derive(Clone, Default)]
pub struct PromptSpec {
    pub message: String,
    pub options: Vec<String>,
    pub default: Option<PromptDefault>,
    pub base: Option<Pt>,
    pub preview: Option<PreviewFn>,
    pub filter: Option<EntFilter>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub integer: bool,
    pub nonzero: bool,
    pub range_message: Option<String>,
    pub allow_enter: bool,
    pub no_enter_ends: bool,
    pub accept_number: bool,
    pub no_snap: bool,
    pub no_ortho: bool,
    /// String prompts: keep surrounding whitespace.
    pub raw: bool,
    pub pickfirst: Option<bool>,
    /// Selection prompts: also pick 3D solids.
    pub solids: bool,
    pub single: bool,
    pub allow_off: bool,
}

impl PromptSpec {
    pub fn new(message: &str) -> PromptSpec {
        PromptSpec { message: message.to_string(), ..PromptSpec::default() }
    }
    pub fn options(mut self, o: &[&str]) -> Self {
        self.options = o.iter().map(|s| s.to_string()).collect();
        self
    }
    pub fn options_vec(mut self, o: Vec<String>) -> Self {
        self.options = o;
        self
    }
    pub fn default_pt(mut self, p: Pt) -> Self {
        self.default = Some(PromptDefault::Pt(p));
        self
    }
    pub fn default_num(mut self, n: f64) -> Self {
        self.default = Some(PromptDefault::Num(n));
        self
    }
    pub fn default_str(mut self, s: &str) -> Self {
        self.default = Some(PromptDefault::Str(s.to_string()));
        self
    }
    pub fn base(mut self, p: Pt) -> Self {
        self.base = Some(p);
        self
    }
    pub fn base_opt(mut self, p: Option<Pt>) -> Self {
        self.base = p;
        self
    }
    pub fn preview(mut self, f: impl Fn(&Drawing, Pt) -> Vec<PreviewItem> + 'static) -> Self {
        self.preview = Some(Rc::new(f));
        self
    }
    pub fn filter(mut self, f: impl Fn(&Entity) -> bool + 'static) -> Self {
        self.filter = Some(Rc::new(f));
        self
    }
    pub fn min(mut self, v: f64) -> Self {
        self.min = Some(v);
        self
    }
    pub fn max(mut self, v: f64) -> Self {
        self.max = Some(v);
        self
    }
    pub fn integer(mut self) -> Self {
        self.integer = true;
        self
    }
    pub fn nonzero(mut self) -> Self {
        self.nonzero = true;
        self
    }
    pub fn range_message(mut self, m: &str) -> Self {
        self.range_message = Some(m.to_string());
        self
    }
    pub fn allow_enter(mut self) -> Self {
        self.allow_enter = true;
        self
    }
    pub fn no_enter_ends(mut self) -> Self {
        self.no_enter_ends = true;
        self
    }
    pub fn accept_number(mut self) -> Self {
        self.accept_number = true;
        self
    }
    pub fn no_snap(mut self) -> Self {
        self.no_snap = true;
        self
    }
    pub fn no_ortho(mut self) -> Self {
        self.no_ortho = true;
        self
    }
    pub fn raw(mut self) -> Self {
        self.raw = true;
        self
    }
    pub fn pickfirst(mut self, v: bool) -> Self {
        self.pickfirst = Some(v);
        self
    }
    pub fn solids(mut self) -> Self {
        self.solids = true;
        self
    }
    pub fn single(mut self) -> Self {
        self.single = true;
        self
    }
    pub fn allow_off(mut self) -> Self {
        self.allow_off = true;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumRel {
    X,
    Xp,
}

/// Raw result of a prompt before it is converted to a typed reply by [`crate::ctx::Ctx`].
#[derive(Clone, Debug)]
pub enum Reply {
    Pt { pt: Pt, typed: bool },
    Num { value: f64, pt: Option<Pt>, rel: Option<NumRel> },
    Text(String),
    Kw(String),
    Enter { pt: Option<Pt>, value: Option<f64>, text: Option<String>, kw: Option<String> },
    Entity { id: u64, pt: Pt, shift: bool },
    Solid { id: u64, pt: Pt },
    Selection(Vec<u64>),
    Dialog(DialogReply),
}

impl Reply {
    pub fn enter() -> Reply {
        Reply::Enter { pt: None, value: None, text: None, kw: None }
    }
}

/// Modal UI requests a command can make (resolved by the front end; headless runs answer `None`).
#[derive(Clone, Debug)]
pub enum DialogReq {
    Insert { names: Vec<String> },
    Color { value: Option<cadforge_core::style::Color>, allow_by_layer: bool },
    Linetype,
    Lweight,
    Hatch,
    DSettings { tab: String },
    /// In-place text editor.
    EditText(TextEditSpec),
    /// Contextual hatch editor bound to the active hatch session; resolves when the user closes it.
    HatchEdit,
    /// Interactive pan / zoom / orbit driven by the mouse until the user stops.
    Realtime(&'static str),
}

/// Where and how the in-place text editor should appear.
#[derive(Clone, Debug)]
pub struct TextEditSpec {
    pub prompt: String,
    pub p: Pt,
    pub h: f64,
    pub rot: f64,
    pub jh: cadforge_core::text::HJust,
    pub jv: cadforge_core::text::VJust,
    pub multi: bool,
    pub w: Option<f64>,
    pub initial: Option<String>,
    pub editing: Option<u64>,
}

#[derive(Clone, Debug)]
pub enum DialogReply {
    Cancelled,
    Text(String),
    Color(Option<cadforge_core::style::Color>),
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FromStage {
    None,
    Base,
    Offset,
}

#[derive(Clone, Debug)]
pub struct WindowSel {
    pub a: Option<Pt>,
    pub mode: Option<char>,
    pub pending: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolyKind {
    Wp,
    Cp,
    Fence,
}

#[derive(Clone, Debug)]
pub struct PolySel {
    pub kind: PolyKind,
    pub pts: Vec<Pt>,
}

/// A prompt waiting for input, plus its transient interaction state.
pub struct Pending {
    pub kind: PromptKind,
    pub spec: PromptSpec,
    pub dialog: Option<DialogReq>,
    pub from_stage: FromStage,
    pub from_base: Option<Pt>,
    pub m2p: Option<Vec<Pt>>,
    /// Distance/angle prompts: the first picked point.
    pub first_point: Option<Pt>,
    pub window: Option<WindowSel>,
    pub polygon: Option<PolySel>,
    pub remove: bool,
    pub single: bool,
    pub allow_off: bool,
    /// Selection prompts: the objects picked so far, in pick order.
    pub set: IndexSet<u64>,
}

impl Pending {
    pub fn new(kind: PromptKind, spec: PromptSpec) -> Pending {
        let single = spec.single;
        let allow_off = spec.allow_off;
        Pending { kind, spec, dialog: None, from_stage: FromStage::None, from_base: None, m2p: None, first_point: None, window: None, polygon: None, remove: false, single, allow_off, set: IndexSet::new() }
    }
    pub fn base(&self) -> Option<Pt> {
        self.spec.base.or(self.first_point)
    }
}

// ---------------------------------------------------------------- runtime state

pub struct TaskState {
    pub def: Rc<crate::registry::CommandDef>,
    pub pending: Option<Pending>,
    pub preview_fn: Option<PreviewFn>,
    pub slot: Option<CmdResult<Reply>>,
    pub fut: Option<CmdFuture>,
    pub transparent: bool,
    pub is_top: bool,
    /// The top-level task whose future contains this one (sub-commands run inline in their parent).
    pub root: TaskId,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// Screen position of the event (needed to pick solids through the camera).
    pub screen: Option<Pt>,
}

#[derive(Clone, Debug)]
pub struct Cursor {
    pub raw: Pt,
    pub pt: Pt,
    pub snap: Option<SnapHit>,
    pub track: Option<Track>,
}

pub struct Runtime {
    pub stack: Vec<TaskId>,
    pub tasks: HashMap<TaskId, TaskState>,
    pub next_task: TaskId,
    pub ready: VecDeque<TaskId>,
    pub snapper: Snapper,
    pub cursor: Cursor,
    pub override_: Option<OsOverride>,
    pub last_command: Option<String>,
    pub recent_commands: Vec<String>,
    pub recent_input: Vec<String>,
    pub preview: Vec<PreviewItem>,
    /// First corner of an idle (no command) selection window.
    pub idle_window: Option<Pt>,
    pub last_point: Option<Pt>,
    pub hot_grip: Option<GripRef>,
    /// Typed text originated from the dynamic-input tooltip.
    pub dynamic_typing: bool,
    pub shift_ortho: bool,
    pub usage: HashMap<String, u32>,
    pub previous_selection: Vec<u64>,
    pub last_window: Option<(Bounds, bool)>,
}

impl Default for Runtime {
    fn default() -> Runtime {
        Runtime {
            stack: vec![],
            tasks: HashMap::new(),
            next_task: 1,
            ready: VecDeque::new(),
            snapper: Snapper::new(),
            cursor: Cursor { raw: pt(0.0, 0.0), pt: pt(0.0, 0.0), snap: None, track: None },
            override_: None,
            last_command: None,
            recent_commands: vec![],
            recent_input: vec![],
            preview: vec![],
            idle_window: None,
            last_point: None,
            hot_grip: None,
            dynamic_typing: false,
            shift_ortho: false,
            usage: HashMap::new(),
            previous_selection: vec![],
            last_window: None,
        }
    }
}

fn fmt(n: f64, p: i32) -> String {
    let k = 10f64.powi(p);
    crate::sysvars::fmt_num((n * k).round() / k)
}

pub fn fmt_pt(p: Pt, prec: i32) -> String {
    format!("{},{}", fmt(p.x, prec), fmt(p.y, prec))
}

fn has_word_or(msg: &str) -> bool {
    msg.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "or")
}

/// What the command line shows for a prompt, e.g. `Specify next point or [Close/Undo]:`.
pub fn prompt_text(p: Option<&Pending>) -> String {
    let Some(p) = p else { return "Command:".into() };
    let mut msg = p.spec.message.clone();
    // a message that already lists its options in brackets is shown as written
    let lists_options = msg.contains('[') && msg.contains(']');
    if !p.spec.options.is_empty() && !lists_options {
        msg += if has_word_or(&msg) || msg.is_empty() { "" } else { " or" };
        msg += " ";
        msg += options_text(&p.spec.options).trim();
    }
    match &p.spec.default {
        Some(d) if p.kind != PromptKind::Keyword => {
            let text = match d {
                PromptDefault::Pt(pt) => fmt_pt(*pt, 4),
                PromptDefault::Num(n) => fmt(if p.kind == PromptKind::Angle { deg(*n) } else { *n }, 4),
                PromptDefault::Str(s) => s.clone(),
            };
            msg += &format!(" <{text}>");
        }
        Some(PromptDefault::Str(s)) if !s.is_empty() => msg += &format!(" <{s}>"),
        _ => {}
    }
    format!("{msg}:")
}

// ---------------------------------------------------------------- the input pipeline

impl App {
    pub fn busy(&self) -> bool {
        !self.rt.stack.is_empty()
    }

    /// The innermost running command.
    pub fn top_task(&self) -> Option<TaskId> {
        self.rt.stack.last().copied()
    }

    pub fn pending(&self) -> Option<&Pending> {
        self.top_task().and_then(|t| self.rt.tasks.get(&t)).and_then(|t| t.pending.as_ref())
    }

    pub fn prompt_line(&self) -> String {
        prompt_text(self.pending())
    }

    /// The modal dialog a command is waiting for, if any (the UI shows it and calls [`App::answer_dialog`]).
    pub fn pending_dialog(&self) -> Option<DialogReq> {
        self.pending().filter(|p| p.kind == PromptKind::Dialog).and_then(|p| p.dialog.clone())
    }

    /// UI → command: answer the pending dialog request.
    pub fn answer_dialog(&mut self, reply: DialogReply) {
        let Some(task) = self.top_task() else { return };
        let is_dialog = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).is_some_and(|p| p.kind == PromptKind::Dialog);
        if is_dialog {
            self.deliver(task, Reply::Dialog(reply));
        }
    }

    pub fn aperture_world(&self, px: Option<f64>) -> f64 {
        let scale = self.editor().map_or(1.0, |e| e.view.scale);
        px.unwrap_or(self.sysvars.real("APERTURE")) / scale
    }

    pub(crate) fn rt_attach(&mut self) {
        self.rt.snapper.reset();
        self.rt.preview.clear();
        self.rt.idle_window = None;
        self.rt.override_ = None;
    }

    pub(crate) fn register_prompt(&mut self, task: TaskId, p: Pending) {
        let is_top = self.top_task() == Some(task);
        let text = prompt_text(Some(&p));
        if let Some(t) = self.rt.tasks.get_mut(&task) {
            t.pending = Some(p);
        }
        if is_top {
            self.log(&text, "prompt");
        }
        self.refresh_preview(false);
        self.emit(Event::Prompt);
        self.emit(Event::Change);
    }

    /// Recompute the rubber-band preview. Pointer moves only need an overlay repaint, not a scene redraw.
    pub fn refresh_preview(&mut self, overlay_only: bool) {
        self.rt.preview.clear();
        let Some(task) = self.top_task() else {
            self.emit(if overlay_only { Event::Overlay } else { Event::Change });
            return;
        };
        let f = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref().and_then(|p| p.spec.preview.clone()).or_else(|| t.preview_fn.clone()));
        if let (Some(f), Some(ed)) = (f, self.editor()) {
            let items = f(&ed.doc, self.rt.cursor.pt);
            self.rt.preview = items;
        }
        self.emit(if overlay_only { Event::Overlay } else { Event::Change });
    }

    /// Resolve a prompt: store the reply and queue the task's root future to resume.
    pub(crate) fn deliver(&mut self, task: TaskId, reply: Reply) {
        let Some(t) = self.rt.tasks.get_mut(&task) else { return };
        let Some(p) = t.pending.take() else { return };
        let reply = if p.kind == PromptKind::Select { Reply::Selection(p.set.iter().copied().collect()) } else { reply };
        t.slot = Some(Ok(reply));
        let root = t.root;
        self.rt.override_ = None;
        self.rt.preview.clear();
        self.rt.ready.push_back(root);
    }

    pub(crate) fn deliver_err(&mut self, task: TaskId, err: CmdError) {
        let Some(t) = self.rt.tasks.get_mut(&task) else { return };
        if t.pending.take().is_none() {
            return;
        }
        t.slot = Some(Err(err));
        let root = t.root;
        self.rt.ready.push_back(root);
    }

    // ------------------------------------------------------------ running commands

    /// Start a command by name/alias. The command runs until its first prompt; call `Handle::pump` (done by the
    /// `Handle` methods) to let queued continuations proceed.
    pub fn run(&mut self, text: &str, args: Args, transparent: bool) -> Option<TaskId> {
        match self.registry.find(text) {
            Some(def) => self.run_def(def, args, transparent),
            None => {
                self.log(&format!("Unknown command \"{text}\".  Press F1 for help."), "error");
                self.emit(Event::Change);
                None
            }
        }
    }

    pub fn run_def(&mut self, def: Rc<crate::registry::CommandDef>, args: Args, transparent: bool) -> Option<TaskId> {
        self.editor()?;
        let is_top = self.rt.stack.is_empty();
        if !is_top && !transparent && !def.transparent {
            self.log("That command may not be used transparently.", "error");
            return None;
        }
        let id = self.begin_task(&def, transparent, is_top, None);
        let ctx = crate::ctx::Ctx::new(self.handle(), id);
        let fut = (def.run)(ctx, args);
        if let Some(t) = self.rt.tasks.get_mut(&id) {
            t.fut = Some(fut);
        }
        self.rt.ready.push_back(id);
        Some(id)
    }

    /// Register a task and push it on the command stack (shared by top-level commands and inline sub-commands).
    pub(crate) fn begin_task(&mut self, def: &Rc<crate::registry::CommandDef>, transparent: bool, is_top: bool, root: Option<TaskId>) -> TaskId {
        let id = self.rt.next_task;
        self.rt.next_task += 1;
        self.rt.tasks.insert(id, TaskState { def: def.clone(), pending: None, preview_fn: None, slot: None, fut: None, transparent, is_top, root: root.unwrap_or(id) });
        self.rt.stack.push(id);
        if !transparent && is_top {
            if let Some(ed) = self.editor_mut() {
                ed.doc.begin_command();
            }
        }
        if !def.silent {
            self.log(&format!("Command: {}", def.name), "cmd");
        }
        *self.rt.usage.entry(def.name.clone()).or_insert(0) += 1;
        id
    }

    /// The `finally` of a command: unwind the stack, close the undo step and report how it ended.
    pub(crate) fn end_task(&mut self, id: TaskId, result: CmdResult<()>) {
        match result {
            Err(CmdError::Cancel) => self.log("*Cancel*", "info"),
            Err(CmdError::Msg(m)) => self.log(&m, "error"),
            Ok(()) => {}
        }
        let Some(t) = self.rt.tasks.remove(&id) else { return };
        self.rt.stack.retain(|x| *x != id);
        self.rt.preview.clear();
        self.rt.override_ = None;
        self.rt.idle_window = None;
        if !t.transparent && t.is_top {
            if let Some(ed) = self.editor_mut() {
                ed.doc.end_command(&t.def.name);
                ed.temp_sel.clear();
                ed.prune();
            }
            self.rt.snapper.reset();
            let name = &t.def.name;
            if t.def.repeatable && !name.starts_with('_') {
                self.rt.last_command = Some(name.clone());
                self.rt.recent_commands.retain(|n| n != name);
                self.rt.recent_commands.insert(0, name.clone());
                self.rt.recent_commands.truncate(12);
            }
            self.emit(Event::CommandEnd(t.def.name.clone()));
        } else if !self.rt.stack.is_empty() {
            self.refresh_preview(false);
        }
        self.emit(Event::Change);
    }

    /// From the UI (ribbon, menus): cancels whatever is running, then starts the command.
    pub fn start(&mut self, name: &str, args: Args) -> Option<TaskId> {
        let def = self.registry.find(name);
        if let Some(def) = &def {
            if def.transparent && self.busy() {
                return self.run_def(def.clone(), args, true);
            }
        }
        if self.busy() {
            // let the cancelled command unwind first; `Handle::pump` starts this one afterwards
            self.escape();
            self.start_deferred(name, args);
            return None;
        }
        self.run(name, args, false)
    }

    // ------------------------------------------------------------ point resolution

    pub fn resolve(&self, raw: Pt, p: Option<&Pending>) -> Resolved {
        let (sv, ed) = (&self.sysvars, self.editor().expect("active editor"));
        let pointy = p.is_some_and(|p| matches!(p.kind, PromptKind::Point | PromptKind::Distance | PromptKind::Angle));
        if !pointy {
            return Resolved { pt: raw, snap: None, track: None };
        }
        let p = p.expect("pointy implies pending");
        let base = p.base();
        let osmask = if p.spec.no_snap { 0 } else { active_mask(sv.int("OSMODE"), self.rt.override_) };
        let ortho = (sv.ortho() != self.rt.shift_ortho) && base.is_some() && !p.spec.no_ortho;
        let o = SnapOptions {
            scale: ed.view.scale,
            aperture_px: sv.real("APERTURE") + 2.0,
            osmask,
            base,
            ortho,
            polar: sv.polar() && base.is_some() && !ortho && !p.spec.no_ortho,
            polar_inc: rad(sv.real("POLARANG")),
            polar_add: vec![],
            otrack: sv.otrack() && !p.spec.no_snap,
            grid: sv.snap_on().then(|| Grid { sx: sv.real("SNAPUNIT_X"), sy: sv.real("SNAPUNIT_Y"), base: ed.doc.ucs.origin, angle: rad(sv.real("SNAPANG")) + ed.doc.ucs.angle }),
            ucs_angle: ed.doc.ucs.angle,
        };
        self.rt.snapper.resolve(&ed.doc, raw, &o)
    }

    /// Mouse move in world coordinates.
    pub fn mouse_move(&mut self, raw: Pt) {
        if self.editor().is_none() {
            return;
        }
        self.rt.cursor.raw = raw;
        let r = {
            let p = self.pending();
            self.resolve(raw, p)
        };
        self.rt.cursor.pt = r.pt;
        self.rt.cursor.snap = r.snap;
        self.rt.cursor.track = r.track;
        let now = self.now;
        let snap = self.rt.cursor.snap;
        self.rt.snapper.note_hover(snap.as_ref(), now);
        self.refresh_preview(true);
    }

    // ------------------------------------------------------------ clicks

    pub fn click(&mut self, raw: Pt, mods: Mods) {
        if self.editor().is_none() {
            return;
        }
        let Some(task) = self.top_task() else {
            self.idle_click(raw, mods);
            return;
        };
        let Some((kind, snapshot)) = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).map(|p| (p.kind, self.resolve(raw, Some(p)))) else { return };
        let r = snapshot;
        self.rt.cursor = Cursor { raw, pt: r.pt, snap: r.snap, track: r.track };
        match kind {
            PromptKind::Point => self.point_picked(task, r.pt),
            PromptKind::Distance | PromptKind::Angle => self.measure_picked(task, r.pt),
            PromptKind::Entity => {
                let filter = self.pending_filter(task);
                let ap = self.aperture_world(None);
                let hit = pick_at(&self.editor().expect("editor").doc, raw, ap, filter.as_deref().map(|f| f as &dyn Fn(&Entity) -> bool), None);
                match hit {
                    Some(id) => self.deliver(task, Reply::Entity { id, pt: raw, shift: mods.shift }),
                    None => self.log("Nothing selected", "error"),
                }
            }
            PromptKind::Solid => match self.solid_at(raw, &mods) {
                Some(id) => self.deliver(task, Reply::Solid { id, pt: raw }),
                None => self.log("Nothing selected", "error"),
            },
            PromptKind::Select => self.select_click(task, raw, mods),
            PromptKind::Keyword | PromptKind::Number | PromptKind::String | PromptKind::Dialog => {}
        }
    }

    fn pending_filter(&self, task: TaskId) -> Option<EntFilter> {
        self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).and_then(|p| p.spec.filter.clone())
    }

    /// Solid under the pointer (screen position from the event, or projected from the world point).
    pub fn solid_at(&self, raw: Pt, mods: &Mods) -> Option<u64> {
        let ed = self.editor()?;
        let doc = &ed.doc;
        if doc.solids.is_empty() {
            return None;
        }
        let sp = mods.screen.unwrap_or_else(|| ed.view.to_screen(raw));
        let wire = ed.visual_style == "2D Wireframe" || ed.visual_style == "Wireframe";
        let aperture = self.sysvars.real("PICKBOX") + 4.0;
        pick_solid(&doc.solids, &ed.view, sp, aperture, !wire, &|s| doc.solid_visible(s) && !doc.layer(if s.layer.is_empty() { "0" } else { &s.layer }).is_some_and(|l| l.locked)).map(|h| h.0)
    }

    pub fn solids_in_rect(&self, a: Pt, b: Pt, crossing: bool) -> Vec<u64> {
        let Some(ed) = self.editor() else { return vec![] };
        let doc = &ed.doc;
        if doc.solids.is_empty() {
            return vec![];
        }
        let (sa, sb) = (ed.view.to_screen(a), ed.view.to_screen(b));
        let rect = Bounds::new(sa.x.min(sb.x), sa.y.min(sb.y), sa.x.max(sb.x), sa.y.max(sb.y));
        solids_in_screen_rect(&doc.solids, &ed.view, &rect, crossing, &|s| doc.solid_visible(s) && !doc.layer(if s.layer.is_empty() { "0" } else { &s.layer }).is_some_and(|l| l.locked))
    }

    fn point_picked(&mut self, task: TaskId, p: Pt) {
        let Some(pend) = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()) else { return };
        match pend.from_stage {
            FromStage::Base => {
                pend.from_base = Some(p);
                pend.from_stage = FromStage::Offset;
                self.log("<Offset>:", "prompt");
                return;
            }
            FromStage::Offset => {
                pend.from_stage = FromStage::None;
                return self.finish_point(task, p);
            }
            FromStage::None => {}
        }
        if let Some(m) = pend.m2p.as_mut() {
            m.push(p);
            if m.len() < 2 {
                self.log("Second point of mid between 2 points:", "prompt");
                return;
            }
            let mid_pt = mid(m[0], m[1]);
            pend.m2p = None;
            return self.finish_point(task, mid_pt);
        }
        self.finish_point(task, p)
    }

    fn finish_point(&mut self, task: TaskId, p: Pt) {
        self.rt.last_point = Some(p);
        self.deliver(task, Reply::Pt { pt: p, typed: false });
    }

    fn measure_picked(&mut self, task: TaskId, p: Pt) {
        let Some(pend) = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()) else { return };
        let Some(base) = pend.base() else {
            pend.first_point = Some(p);
            self.log("Specify second point:", "prompt");
            self.refresh_preview(false);
            return;
        };
        let value = if pend.kind == PromptKind::Distance { dist(base, p) } else { angle_of(base, p) };
        self.deliver(task, Reply::Num { value, pt: Some(p), rel: None });
    }

    fn idle_click(&mut self, raw: Pt, mods: Mods) {
        if let Some(a) = self.rt.idle_window.take() {
            self.window_select(a, raw, mods);
            return;
        }
        if !mods.shift {
            if let Some(grip) = self.grip_at(raw) {
                self.run("_GRIPEDIT", Args::none().with("grip", ArgVal::Grip(grip)), false);
                return;
            }
        }
        let ap = self.aperture_world(Some(self.sysvars.real("PICKBOX") + 4.0));
        let hit = pick_at(&self.editor().expect("editor").doc, raw, ap, None, None);
        if let Some(id) = hit {
            let ids = self.group_ids(id);
            let ed = self.editor_mut().expect("editor");
            if mods.shift {
                let all = ids.iter().all(|i| ed.sel.contains(i));
                for i in ids {
                    if all {
                        ed.sel.shift_remove(&i);
                    } else {
                        ed.sel.insert(i);
                    }
                }
            } else {
                ed.sel.extend(ids);
            }
            self.emit(Event::Change);
            return;
        }
        if let Some(sid) = self.solid_at(raw, &mods) {
            let ed = self.editor_mut().expect("editor");
            if mods.shift && ed.sel.contains(&sid) {
                ed.sel.shift_remove(&sid);
            } else {
                ed.sel.insert(sid);
            }
            self.emit(Event::Change);
            return;
        }
        self.rt.idle_window = Some(raw);
        self.emit(Event::Change);
    }

    /// Ids of the entities sharing a group with `id` (GROUP selection), including `id` itself.
    pub fn group_ids(&self, id: u64) -> Vec<u64> {
        let Some(ed) = self.editor() else { return vec![id] };
        match ed.entity_by_id(id).and_then(|e| e.grp.clone()) {
            None => vec![id],
            Some(g) => ed.doc.entities().iter().filter(|x| x.grp.as_ref() == Some(&g)).map(|x| x.id).collect(),
        }
    }

    pub fn window_select(&mut self, a: Pt, b: Pt, mods: Mods) {
        let crossing = b.x < a.x;
        let ids: IndexSet<u64> = {
            let ed = self.editor().expect("editor");
            let list = select_rect(&ed.doc, &rect_of(a, b), crossing, None);
            let mut ids: IndexSet<u64> = list.iter().flat_map(|id| self.group_ids(*id)).collect();
            ids.extend(self.solids_in_rect(a, b, crossing));
            ids
        };
        let n = ids.len();
        let ed = self.editor_mut().expect("editor");
        if mods.shift {
            for id in &ids {
                ed.sel.shift_remove(id);
            }
        } else {
            ed.sel.extend(ids);
        }
        self.log(&format!("{n} found"), "info");
        self.emit(Event::Change);
    }

    /// Mouse drag (window/crossing) from a to b.
    pub fn drag(&mut self, a: Pt, b: Pt, mods: Mods) {
        let Some(task) = self.top_task() else {
            self.rt.idle_window = None;
            self.window_select(a, b, mods);
            return;
        };
        if self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).is_some_and(|p| p.kind == PromptKind::Select) {
            self.select_rect_prompt(task, a, b, mods, None);
        }
    }

    /// Double click: edit the object under the cursor.
    pub fn double_click(&mut self, raw: Pt) {
        if self.busy() || self.editor().is_none() {
            return;
        }
        if self.double_click_space(raw) {
            return;
        }
        let ap = self.aperture_world(None);
        let hit = pick_at(&self.editor().expect("editor").doc, raw, ap, None, None);
        let Some(id) = hit else {
            if let Some(s) = self.solid_at(raw, &Mods::default()) {
                let ed = self.editor_mut().expect("editor");
                ed.sel.clear();
                ed.sel.insert(s);
                self.start("PROPERTIES", Args::none());
            }
            return;
        };
        let cmd = {
            let ed = self.editor_mut().expect("editor");
            ed.sel.clear();
            ed.sel.insert(id);
            match ed.entity_by_id(id).map(|e| e.type_name()) {
                Some("text") | Some("dim") | Some("leader") => "TEXTEDIT",
                Some("pline") => "PEDIT",
                Some("hatch") => "HATCHEDIT",
                _ => "PROPERTIES",
            }
        };
        self.start(cmd, Args::none().with("entity", ArgVal::Id(id)));
    }

    // ------------------------------------------------------------ grips

    pub fn grip_at(&self, raw: Pt) -> Option<GripRef> {
        let ed = self.editor()?;
        if ed.sel.is_empty() || self.sysvars.int("GRIPS") == 0 || ed.sel.len() as i64 > self.sysvars.int("GRIPOBJLIMIT") {
            return None;
        }
        let tol = 7.0 / ed.view.scale;
        for e in ed.selected() {
            if ed.doc.is_locked(e) {
                continue;
            }
            for g in grips_of(e, &ed.doc) {
                if dist(g.pt, raw) <= tol {
                    return Some(GripRef { entity: e.id, id: g.id, pt: g.pt, kind: g.kind });
                }
            }
        }
        None
    }

    // ------------------------------------------------------------ selection prompt

    /// May the entity be picked by this selection prompt (visible, unlocked, passes the filter)?
    pub fn selectable_by(&self, e: &Entity, allow_off: bool, filter: Option<&EntFilter>) -> bool {
        let Some(ed) = self.editor() else { return false };
        let d = &ed.doc;
        let l = d.layer_of(e);
        (if allow_off { !l.frozen } else { d.is_visible(e) }) && !l.locked && filter.map_or(true, |f| f(e))
    }

    fn sel_add(&mut self, task: TaskId, list: &[u64]) {
        let (remove, filter, allow_off) = match self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()) {
            Some(p) => (p.remove, p.spec.filter.clone(), p.allow_off),
            None => return,
        };
        let mut candidates: Vec<u64> = vec![];
        for &id in list {
            let ok = self.editor().and_then(|ed| ed.entity_by_id(id)).is_some_and(|e| self.selectable_by(e, allow_off, filter.as_ref()));
            if !ok {
                continue;
            }
            for gid in self.group_ids(id) {
                if self.editor().and_then(|ed| ed.entity_by_id(gid)).is_some_and(|m| self.selectable_by(m, allow_off, filter.as_ref())) {
                    candidates.push(gid);
                }
            }
        }
        let Some(p) = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()) else { return };
        let mut added = 0;
        for gid in candidates {
            if remove {
                if p.set.shift_remove(&gid) {
                    added += 1;
                }
            } else if p.set.insert(gid) {
                added += 1;
            }
        }
        let (total, single, keys): (usize, bool, Vec<u64>) = (p.set.len(), p.single, p.set.iter().copied().collect());
        let msg = if remove { format!("{added} removed, {total} total") } else { format!("{added} found{}", if total != added { format!(", {total} total") } else { String::new() }) };
        self.editor_mut().expect("editor").temp_sel = keys.iter().copied().collect();
        self.log(&msg, "info");
        self.emit(Event::Change);
        if single && total > 0 {
            self.rt.previous_selection = keys;
            self.deliver(task, Reply::enter());
        }
    }

    fn sel_add_solids(&mut self, task: TaskId, solids: &[u64], remove: bool) {
        let Some(p) = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()) else { return };
        let mut n = 0;
        for &s in solids {
            if remove || p.remove {
                if p.set.shift_remove(&s) {
                    n += 1;
                }
            } else if p.set.insert(s) {
                n += 1;
            }
        }
        let (total, single, keys): (usize, bool, Vec<u64>) = (p.set.len(), p.single, p.set.iter().copied().collect());
        self.editor_mut().expect("editor").temp_sel = keys.into_iter().collect();
        self.log(&format!("{n} found{}", if total != n { format!(", {total} total") } else { String::new() }), "info");
        self.emit(Event::Change);
        if single && total > 0 {
            self.deliver(task, Reply::enter());
        }
    }

    fn select_click(&mut self, task: TaskId, raw: Pt, mods: Mods) {
        let Some(p) = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()) else { return };
        if let Some(w) = p.window.as_mut() {
            if w.a.is_none() {
                w.a = Some(raw);
                self.log("Specify opposite corner:", "prompt");
                self.emit(Event::Change);
                return;
            }
            let (a, mode) = (w.a.expect("first corner"), w.mode);
            p.window = None;
            self.select_rect_prompt(task, a, raw, mods, mode);
            return;
        }
        if let Some(poly) = p.polygon.as_mut() {
            poly.pts.push(raw);
            let what = if poly.kind == PolyKind::Fence { "line" } else { "polygon" };
            self.log(&format!("Specify endpoint of {what} or [Undo]:"), "prompt");
            self.emit(Event::Change);
            return;
        }
        let (filter, solids, single) = (p.spec.filter.clone(), p.spec.solids, p.single);
        let ap = self.aperture_world(Some(self.sysvars.real("PICKBOX") + 4.0));
        let hit = pick_at(&self.editor().expect("editor").doc, raw, ap, filter.as_deref().map(|f| f as &dyn Fn(&Entity) -> bool), None);
        if let Some(id) = hit {
            if mods.shift {
                let prev = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()).map(|p| std::mem::replace(&mut p.remove, true));
                self.sel_add(task, &[id]);
                if let (Some(pr), Some(p)) = (prev, self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut())) {
                    p.remove = pr;
                }
            } else {
                self.sel_add(task, &[id]);
            }
            return;
        }
        if solids {
            if let Some(s) = self.solid_at(raw, &mods) {
                self.sel_add_solids(task, &[s], mods.shift);
                return;
            }
        }
        if single {
            self.log("Nothing selected", "error");
            return;
        }
        if let Some(p) = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()) {
            p.window = Some(WindowSel { a: Some(raw), mode: None, pending: false });
        }
        self.log("Specify opposite corner:", "prompt");
        self.emit(Event::Change);
    }

    fn select_rect_prompt(&mut self, task: TaskId, a: Pt, b: Pt, mods: Mods, mode: Option<char>) {
        let crossing = match mode {
            Some(m) => m == 'C',
            None => b.x < a.x,
        };
        let Some(p) = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()) else { return };
        let (filter, solids) = (p.spec.filter.clone(), p.spec.solids);
        let list = select_rect(&self.editor().expect("editor").doc, &rect_of(a, b), crossing, filter.as_deref().map(|f| f as &dyn Fn(&Entity) -> bool));
        self.rt.last_window = Some((rect_of(a, b), crossing));
        let prev = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()).map(|p| p.remove);
        if mods.shift {
            if let Some(p) = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()) {
                p.remove = true;
            }
        }
        self.sel_add(task, &list);
        if let (Some(pr), Some(p)) = (prev, self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut())) {
            p.remove = pr;
        }
        if solids {
            let s = self.solids_in_rect(a, b, crossing);
            self.sel_add_solids(task, &s, mods.shift);
        }
    }

    fn select_text(&mut self, task: TaskId, text: &str) {
        let t = text.trim().to_uppercase();
        // polygon / fence point entry
        let in_polygon = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).is_some_and(|p| p.polygon.is_some());
        if in_polygon {
            if t.is_empty() {
                self.poly_done(task);
                return;
            }
            if t == "U" || t == "UNDO" {
                if let Some(poly) = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()).and_then(|p| p.polygon.as_mut()) {
                    poly.pts.pop();
                }
                return;
            }
            let base = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).and_then(|p| p.polygon.as_ref()).and_then(|g| g.pts.last().copied());
            match parse_input(text, InputOpts { base, relative_default: false, ucs: self.ucs() }) {
                Some(Input::Point { pt: p, .. }) => {
                    if let Some(poly) = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()).and_then(|p| p.polygon.as_mut()) {
                        poly.pts.push(p);
                    }
                    self.emit(Event::Change);
                }
                _ => self.log("Invalid point.", "error"),
            }
            return;
        }
        if let Some(Input::Point { pt: p, .. }) = parse_input(text, InputOpts { ucs: self.ucs(), ..InputOpts::default() }) {
            self.select_click(task, p, Mods::default());
            return;
        }
        match t.as_str() {
            "ALL" => {
                // ALL picks everything on thawed, unlocked layers, including objects on layers that are switched off
                let ids: Vec<u64> = self.editor().expect("editor").doc.entities().iter().filter(|e| {
                    let l = self.editor().expect("editor").doc.layer_of(e);
                    !l.frozen && !l.locked
                }).map(|e| e.id).collect();
                self.set_pending_flag(task, |p| p.allow_off = true);
                self.sel_add(task, &ids);
                self.set_pending_flag(task, |p| p.allow_off = false);
                if self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).is_some_and(|p| p.spec.solids) {
                    let solids: Vec<u64> = self.doc().map_or(vec![], |d| d.solids.iter().filter(|s| d.solid_visible(s) && !d.layer(if s.layer.is_empty() { "0" } else { &s.layer }).is_some_and(|l| l.locked)).map(|s| s.id).collect());
                    if !solids.is_empty() {
                        self.sel_add_solids(task, &solids, false);
                    }
                }
            }
            "W" | "WINDOW" | "C" | "CROSSING" => {
                let mode = if t.starts_with('W') { 'W' } else { 'C' };
                self.set_pending_flag(task, |p| p.window = Some(WindowSel { a: None, mode: Some(mode), pending: true }));
                self.log("Specify first corner:", "prompt");
            }
            "WP" | "CP" => {
                let kind = if t == "WP" { PolyKind::Wp } else { PolyKind::Cp };
                self.set_pending_flag(task, |p| p.polygon = Some(PolySel { kind, pts: vec![] }));
                self.log("First polygon point:", "prompt");
            }
            "F" | "FENCE" => {
                self.set_pending_flag(task, |p| p.polygon = Some(PolySel { kind: PolyKind::Fence, pts: vec![] }));
                self.log("First fence point:", "prompt");
            }
            "L" | "LAST" => {
                let filter = self.pending_filter(task);
                let allow_off = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).is_some_and(|p| p.allow_off);
                let last = self.editor().expect("editor").doc.entities().iter().rev().find(|e| self.selectable_by(e, allow_off, filter.as_ref())).map(|e| e.id);
                match last {
                    Some(id) => self.sel_add(task, &[id]),
                    None => self.log("0 found", "info"),
                }
            }
            "P" | "PREVIOUS" => {
                let prev: Vec<u64> = self.rt.previous_selection.iter().copied().filter(|id| self.editor().expect("editor").entity_by_id(*id).is_some()).collect();
                self.sel_add(task, &prev);
            }
            "R" | "REMOVE" => {
                self.set_pending_flag(task, |p| p.remove = true);
                self.log("Remove objects:", "prompt");
            }
            "A" | "ADD" => {
                self.set_pending_flag(task, |p| p.remove = false);
                self.log("Select objects:", "prompt");
            }
            "U" | "UNDO" => {
                let keys = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()).map(|p| {
                    p.set.pop();
                    p.set.iter().copied().collect::<IndexSet<u64>>()
                });
                if let Some(k) = keys {
                    self.editor_mut().expect("editor").temp_sel = k;
                    self.emit(Event::Change);
                }
            }
            "SI" | "SINGLE" => self.set_pending_flag(task, |p| p.single = true),
            "M" | "MULTIPLE" | "ALLSEL" => {}
            _ => self.log("*Invalid selection* Expects a point or Window/Last/Crossing/BOX/ALL/Fence/WPolygon/CPolygon/Add/Remove/Multiple/Previous/Undo/SIngle", "error"),
        }
    }

    fn set_pending_flag(&mut self, task: TaskId, f: impl FnOnce(&mut Pending)) {
        if let Some(p) = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()) {
            f(p);
        }
    }

    fn poly_done(&mut self, task: TaskId) {
        let Some(p) = self.rt.tasks.get_mut(&task).and_then(|t| t.pending.as_mut()) else { return };
        let Some(pg) = p.polygon.take() else { return };
        let filter = p.spec.filter.clone();
        let need = if pg.kind == PolyKind::Fence { 2 } else { 3 };
        if pg.pts.len() < need {
            return;
        }
        let f = filter.as_deref().map(|f| f as &dyn Fn(&Entity) -> bool);
        let doc = &self.editor().expect("editor").doc;
        let list = match pg.kind {
            PolyKind::Fence => select_fence(doc, &pg.pts, f),
            PolyKind::Cp => select_polygon(doc, &pg.pts, true, f),
            PolyKind::Wp => select_polygon(doc, &pg.pts, false, f),
        };
        self.sel_add(task, &list);
    }

    // ------------------------------------------------------------ typed input

    /// Text submitted from the command line (or dynamic input).
    pub fn text(&mut self, raw: &str, dynamic: bool) {
        self.rt.dynamic_typing = dynamic;
        let Some(task) = self.top_task() else {
            self.idle_text(raw);
            return;
        };
        let Some((kind, options)) = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).map(|p| (p.kind, p.spec.options.clone())) else { return };
        if raw.trim().is_empty() && kind != PromptKind::String {
            self.enter();
            return;
        }
        let t = raw.trim().to_string();
        self.rt.recent_input.retain(|x| *x != t);
        self.rt.recent_input.insert(0, t.clone());
        self.rt.recent_input.truncate(20);
        self.log(raw, "input");
        // transparent apostrophe commands
        if let Some(rest) = t.strip_prefix('\'') {
            if !rest.is_empty() {
                self.run(rest, Args::none(), true);
                return;
            }
        }
        match kind {
            PromptKind::Select => self.select_text(task, raw),
            PromptKind::String => {
                let (default, rawflag) = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).map(|p| (p.spec.default.clone(), p.spec.raw)).unwrap_or((None, false));
                let reply = match (raw.is_empty(), default) {
                    (true, Some(PromptDefault::Str(d))) => Reply::Enter { pt: None, value: None, text: Some(d), kw: None },
                    _ => Reply::Text(if rawflag { raw.to_string() } else { t.clone() }),
                };
                self.deliver(task, reply);
            }
            PromptKind::Keyword => match match_option(&t, &options) {
                Some(kw) => {
                    let kw = kw.clone();
                    self.deliver(task, Reply::Kw(kw))
                }
                None => self.log("Invalid option keyword.", "error"),
            },
            PromptKind::Number => self.number_text(task, &t),
            PromptKind::Distance | PromptKind::Angle => self.measure_text(task, &t),
            PromptKind::Entity => match match_option(&t, &options) {
                Some(kw) => {
                    let kw = kw.clone();
                    self.deliver(task, Reply::Kw(kw))
                }
                None => self.log("Select an object or option keyword.", "error"),
            },
            PromptKind::Point => self.point_text(task, &t),
            PromptKind::Solid | PromptKind::Dialog => {}
        }
    }

    fn point_text(&mut self, task: TaskId, t: &str) {
        let Some(p) = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()) else { return };
        if let Some(kw) = match_option(t, &p.spec.options) {
            let kw = kw.clone();
            return self.deliver(task, Reply::Kw(kw));
        }
        if let Some(ov) = parse_override(t) {
            match ov {
                Override::Osnap(code) => {
                    self.rt.override_ = Some(OsOverride::Mask(os_bit(code)));
                    self.log(&format!("_{} of", code.to_lowercase()), "prompt");
                }
                Override::None => {
                    self.rt.override_ = Some(OsOverride::None);
                    self.log("_non", "prompt");
                }
                Override::From => {
                    self.set_pending_flag(task, |p| p.from_stage = FromStage::Base);
                    self.log("_from Base point:", "prompt");
                }
                Override::M2p => {
                    self.set_pending_flag(task, |p| p.m2p = Some(vec![]));
                    self.log("First point of mid between 2 points:", "prompt");
                }
                Override::Tt => self.log("Temporary tracking is not supported; use object snap tracking.", "error"),
            }
            return;
        }
        let p = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).expect("pending");
        let (from_stage, from_base, base, accept_number, has_base) = (p.from_stage, p.from_base, p.spec.base, p.spec.accept_number, p.spec.base.is_some());
        let rel = self.rt.dynamic_typing && self.sysvars.pointer_input() && has_base && from_stage != FromStage::Offset;
        if accept_number {
            if let Some((v, r)) = scale_suffix(t) {
                return self.deliver(task, Reply::Num { value: v, pt: None, rel: Some(r) });
            }
        }
        let origin = if from_stage == FromStage::Offset { from_base } else { base };
        let input = parse_input(t, InputOpts { base: origin, relative_default: rel || from_stage == FromStage::Offset, ucs: self.ucs() });
        let Some(input) = input else {
            self.log("Point or option keyword required.", "error");
            return;
        };
        if from_stage == FromStage::Base {
            let p = match input {
                Input::Point { pt, .. } => pt,
                _ => self.rt.cursor.pt,
            };
            self.set_pending_flag(task, |q| {
                q.from_base = Some(p);
                q.from_stage = FromStage::Offset;
            });
            self.log("<Offset>:", "prompt");
            return;
        }
        match input {
            Input::Point { pt: p, .. } => {
                self.set_pending_flag(task, |q| q.from_stage = FromStage::None);
                self.rt.last_point = Some(p);
                self.deliver(task, Reply::Pt { pt: p, typed: true });
            }
            Input::Number(v) => {
                // bare number: scale factors (ZOOM), or direct distance entry along the current cursor direction
                if accept_number {
                    return self.deliver(task, Reply::Num { value: v, pt: None, rel: None });
                }
                let Some(b) = base else {
                    self.log("Point or option keyword required.", "error");
                    return;
                };
                let cur = self.rt.cursor.pt;
                let dir = if dist(b, cur) < 1e-12 { pt(1.0, 0.0) } else { unit(cur - b) };
                let p = b + dir * v;
                self.rt.last_point = Some(p);
                self.deliver(task, Reply::Pt { pt: p, typed: true });
            }
        }
    }

    fn number_text(&mut self, task: TaskId, t: &str) {
        let Some(p) = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()) else { return };
        if let Some(kw) = match_option(t, &p.spec.options) {
            let kw = kw.clone();
            return self.deliver(task, Reply::Kw(kw));
        }
        let Some(Input::Number(v)) = parse_input(t, InputOpts::default()) else {
            self.log("Requires numeric value.", "error");
            return;
        };
        let s = &p.spec;
        if s.integer && v.fract() != 0.0 {
            self.log("Requires an integer value.", "error");
            return;
        }
        if s.min.is_some_and(|m| v < m) || s.max.is_some_and(|m| v > m) || (s.nonzero && v == 0.0) {
            let m = s.range_message.clone().unwrap_or_else(|| "*Invalid* Value out of range.".into());
            self.log(&m, "error");
            return;
        }
        self.deliver(task, Reply::Num { value: v, pt: None, rel: None });
    }

    fn measure_text(&mut self, task: TaskId, t: &str) {
        let Some(p) = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()) else { return };
        if let Some(kw) = match_option(t, &p.spec.options) {
            let kw = kw.clone();
            return self.deliver(task, Reply::Kw(kw));
        }
        let (kind, min, nonzero, base, first) = (p.kind, p.spec.min, p.spec.nonzero, p.spec.base, p.first_point);
        match parse_input(t, InputOpts { base, relative_default: false, ucs: self.ucs() }) {
            Some(Input::Number(v)) => {
                if min.is_some_and(|m| v < m) {
                    self.log("Requires a positive value.", "error");
                    return;
                }
                if nonzero && v == 0.0 {
                    self.log("Requires a nonzero value.", "error");
                    return;
                }
                self.deliver(task, Reply::Num { value: if kind == PromptKind::Angle { rad(v) } else { v }, pt: None, rel: None });
            }
            Some(Input::Point { pt: q, .. }) => {
                // a typed point sets the base, then measure to the next
                match base.or(first) {
                    None => {
                        self.set_pending_flag(task, |p| p.first_point = Some(q));
                        self.log("Specify second point:", "prompt");
                    }
                    Some(b) => {
                        let value = if kind == PromptKind::Distance { dist(b, q) } else { angle_of(b, q) };
                        self.deliver(task, Reply::Num { value, pt: Some(q), rel: None });
                    }
                }
            }
            None => self.log(if kind == PromptKind::Angle { "Requires numeric angle or second point." } else { "Requires numeric distance or second point." }, "error"),
        }
    }

    // ------------------------------------------------------------ Enter / Escape

    pub fn enter(&mut self) {
        let Some(task) = self.top_task() else {
            self.repeat();
            return;
        };
        let Some(p) = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()) else { return };
        let kind = p.kind;
        let (default, allow_enter, no_enter_ends) = (p.spec.default.clone(), p.spec.allow_enter, p.spec.no_enter_ends);
        match kind {
            PromptKind::Select => {
                if p.window.as_ref().is_some_and(|w| w.pending) {
                    self.set_pending_flag(task, |p| p.window = None);
                }
                if self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).is_some_and(|p| p.polygon.is_some()) {
                    self.poly_done(task);
                    return;
                }
                let keys: Vec<u64> = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()).map(|p| p.set.iter().copied().collect()).unwrap_or_default();
                if !keys.is_empty() {
                    self.rt.previous_selection = keys;
                }
                self.deliver(task, Reply::enter());
            }
            PromptKind::String => {
                let text = match default {
                    Some(PromptDefault::Str(s)) => s,
                    _ => String::new(),
                };
                self.deliver(task, Reply::Enter { pt: None, value: None, text: Some(text), kw: None })
            }
            PromptKind::Dialog => {}
            _ if default.is_some() && kind != PromptKind::Keyword => {
                let reply = match default.expect("default") {
                    PromptDefault::Pt(p) if kind == PromptKind::Point => Reply::Enter { pt: Some(p), value: None, text: None, kw: None },
                    PromptDefault::Num(n) => Reply::Enter { pt: None, value: Some(n), text: None, kw: None },
                    PromptDefault::Pt(_) | PromptDefault::Str(_) => Reply::enter(),
                };
                self.deliver(task, reply)
            }
            PromptKind::Keyword => {
                let kw = match default {
                    Some(PromptDefault::Str(s)) if !s.is_empty() => Some(s),
                    _ => None,
                };
                self.deliver(task, Reply::Enter { pt: None, value: None, text: None, kw })
            }
            _ if allow_enter || kind == PromptKind::Entity || no_enter_ends => self.deliver(task, Reply::enter()),
            _ => self.log(if kind == PromptKind::Point { "Point or option keyword required." } else { "Invalid input." }, "error"),
        }
    }

    /// A keyword chosen from a menu / clicked in the command line.
    pub fn keyword(&mut self, label: &str) {
        let Some(task) = self.top_task() else { return };
        let Some(p) = self.rt.tasks.get(&task).and_then(|t| t.pending.as_ref()) else { return };
        if p.kind == PromptKind::Select {
            self.select_text(task, label);
            return;
        }
        if let Some(kw) = match_option(label, &p.spec.options) {
            let kw = kw.clone();
            self.log(option_name(&kw), "input");
            self.deliver(task, Reply::Kw(kw));
        }
    }

    pub fn escape(&mut self) {
        self.rt.idle_window = None;
        self.rt.override_ = None;
        if self.top_task().is_none() {
            if let Some(ed) = self.editor_mut() {
                ed.clear_sel();
            }
            self.rt.hot_grip = None;
            self.emit(Event::Change);
            return;
        }
        let stack: Vec<TaskId> = self.rt.stack.iter().rev().copied().collect();
        for t in stack {
            self.deliver_err(t, CmdError::Cancel);
        }
        self.rt.preview.clear();
        self.emit(Event::Change);
    }

    fn repeat(&mut self) {
        if let Some(c) = self.rt.last_command.clone() {
            self.run(&c, Args::none(), false);
        }
    }

    // ------------------------------------------------------------ idle typing: commands, aliases, system variables

    fn idle_text(&mut self, text: &str) {
        let t = text.trim();
        if t.is_empty() {
            self.repeat();
            return;
        }
        self.log(&format!("Command: {t}"), "input");
        let name = t.trim_start_matches('\'');
        if self.registry.find(name).is_some() {
            self.run(name, Args::none(), false);
            return;
        }
        if self.sysvars.has(name) {
            self.run("SETVAR", Args::none().with("name", ArgVal::Str(name.to_uppercase())), false);
            return;
        }
        self.log(&format!("Unknown command \"{t}\".  Press F1 for help."), "error");
        self.emit(Event::Change);
    }
}

/// `2x` / `0.5xp` scale-factor typed at a point prompt that accepts numbers (ZOOM).
fn scale_suffix(t: &str) -> Option<(f64, NumRel)> {
    let lower = t.to_lowercase();
    let (body, rel) = if let Some(b) = lower.strip_suffix("xp") {
        (b, NumRel::Xp)
    } else if let Some(b) = lower.strip_suffix('x') {
        (b, NumRel::X)
    } else {
        return None;
    };
    let body = body.trim();
    match parse_input(body, InputOpts::default()) {
        Some(Input::Number(v)) if !body.contains(',') => Some((v, rel)),
        _ => None,
    }
}
