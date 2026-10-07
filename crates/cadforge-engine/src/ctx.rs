//! The command context: how an `async fn` command talks to the runtime (port of `Ctx` in `runtime.js`).
//!
//! `Ctx` is a cheap handle. Methods that touch application state take closures (`ctx.doc(|d| ..)`), so a borrow can
//! never be held across an `.await`.
use crate::app::{App, Event, Handle};
use crate::editor::Editor;
use crate::runtime::*;
use crate::sysvars::Value;
use crate::view::View;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::Entity;
use cadforge_core::geom::Pt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Clone)]
pub struct Ctx {
    pub h: Handle,
    pub task: TaskId,
}

// ---------------------------------------------------------------- typed replies

#[derive(Clone, Debug, PartialEq)]
pub enum PointR {
    Pt(Pt),
    Kw(String),
    /// Enter pressed (with the prompt's default point, if it had one).
    Enter(Option<Pt>),
    /// A bare number (only for prompts created with `accept_number`).
    Num { value: f64, rel: Option<NumRel> },
}

impl PointR {
    /// The point picked, or the default when Enter was pressed.
    pub fn pt(&self) -> Option<Pt> {
        match self {
            PointR::Pt(p) => Some(*p),
            PointR::Enter(p) => *p,
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NumR {
    Value(f64),
    Kw(String),
    Enter(Option<f64>),
}

impl NumR {
    /// The number typed, or the default when Enter was pressed.
    pub fn value(&self) -> Option<f64> {
        match self {
            NumR::Value(v) => Some(*v),
            NumR::Enter(v) => *v,
            NumR::Kw(_) => None,
        }
    }
}

/// Distance / angle reply: the value, plus the point that was picked to measure it.
#[derive(Clone, Debug, PartialEq)]
pub enum MeasR {
    Value { value: f64, pt: Option<Pt> },
    Kw(String),
    Enter(Option<f64>),
}

impl MeasR {
    pub fn value(&self) -> Option<f64> {
        match self {
            MeasR::Value { value, .. } => Some(*value),
            MeasR::Enter(v) => *v,
            MeasR::Kw(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrR {
    pub text: String,
    /// Enter was pressed (text is then the default).
    pub entered: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum KeyR {
    Kw(String),
    Enter(Option<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum EntR {
    Entity { id: u64, pt: Pt, shift: bool },
    Kw(String),
    Enter,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SolidR {
    Solid { id: u64, pt: Pt },
    Kw(String),
    Enter,
}

/// Objects picked by a "Select objects" prompt.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    pub ents: Vec<u64>,
    pub solids: Vec<u64>,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.ents.is_empty() && self.solids.is_empty()
    }
    pub fn len(&self) -> usize {
        self.ents.len() + self.solids.len()
    }
}

fn unexpected<T>() -> CmdResult<T> {
    Err(CmdError::Msg("Unexpected reply from the prompt.".into()))
}

// ---------------------------------------------------------------- awaiting a prompt

struct AskFuture {
    ctx: Ctx,
    pending: Option<Pending>,
}

impl Future for AskFuture {
    type Output = CmdResult<Reply>;
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        if let Some(p) = this.pending.take() {
            this.ctx.h.0.borrow_mut().register_prompt(this.ctx.task, p);
            return Poll::Pending;
        }
        let mut app = this.ctx.h.0.borrow_mut();
        match app.rt.tasks.get_mut(&this.ctx.task).and_then(|t| t.slot.take()) {
            Some(r) => Poll::Ready(r),
            None => Poll::Pending,
        }
    }
}

impl Ctx {
    pub fn new(h: Handle, task: TaskId) -> Ctx {
        Ctx { h, task }
    }

    // ------------------------------------------------------------ state access

    pub fn app<R>(&self, f: impl FnOnce(&mut App) -> R) -> R {
        f(&mut self.h.0.borrow_mut())
    }

    pub fn ed<R>(&self, f: impl FnOnce(&mut Editor) -> R) -> R {
        let mut a = self.h.0.borrow_mut();
        f(a.editor_mut().expect("a command runs on an open drawing"))
    }

    pub fn doc<R>(&self, f: impl FnOnce(&Drawing) -> R) -> R {
        let a = self.h.0.borrow();
        f(&a.editor().expect("a command runs on an open drawing").doc)
    }

    /// Mutable access to the drawing. The drawing methods (`add`, `replace`, `change`...) take the undo snapshot
    /// themselves; use `d.change(|d| ..)` for in-place edits.
    pub fn doc_mut<R>(&self, f: impl FnOnce(&mut Drawing) -> R) -> R {
        let mut a = self.h.0.borrow_mut();
        f(&mut a.editor_mut().expect("a command runs on an open drawing").doc)
    }

    pub fn view<R>(&self, f: impl FnOnce(&mut View) -> R) -> R {
        self.ed(|e| f(&mut e.view))
    }

    pub fn name(&self) -> String {
        self.app(|a| a.rt.tasks.get(&self.task).map(|t| t.def.name.clone()).unwrap_or_default())
    }

    pub fn log(&self, text: &str) {
        self.app(|a| a.log(text, "info"));
    }

    pub fn log_error(&self, text: &str) {
        self.app(|a| a.log(text, "error"));
    }

    pub fn emit(&self) {
        self.app(|a| a.emit(Event::Change));
    }

    pub fn var(&self, name: &str) -> Result<Value, String> {
        self.app(|a| {
            let doc = a.editors.get(a.active.unwrap_or(0)).map(|e| &e.doc);
            a.sysvars.get(name, doc)
        })
    }

    pub fn setvar(&self, name: &str, v: impl Into<Value>) -> Result<(), String> {
        let v = v.into();
        self.app(|a| {
            let i = a.active.unwrap_or(0);
            let doc = a.editors.get_mut(i).map(|e| &mut e.doc);
            a.sysvars.set(name, v, doc)
        })
    }

    pub fn pref(&self, name: &str) -> f64 {
        self.app(|a| a.sysvars.real(name))
    }

    pub fn set_preview(&self, f: impl Fn(&Drawing, Pt) -> Vec<PreviewItem> + 'static) {
        let rc: PreviewFn = std::rc::Rc::new(f);
        self.app(|a| {
            if let Some(t) = a.rt.tasks.get_mut(&self.task) {
                t.preview_fn = Some(rc);
            }
            a.refresh_preview(false);
        });
    }

    pub fn clear_preview(&self) {
        self.app(|a| {
            if let Some(t) = a.rt.tasks.get_mut(&self.task) {
                t.preview_fn = None;
            }
            a.refresh_preview(false);
        });
    }

    pub fn clear_sel(&self) {
        self.app(|a| {
            if let Some(e) = a.editor_mut() {
                e.clear_sel();
            }
            a.emit(Event::Change);
        });
    }

    pub fn commit_change(&self) {
        self.ed(|e| e.prune());
    }

    /// Add an entity to the active space; returns its id.
    pub fn add(&self, e: Entity) -> u64 {
        self.doc_mut(|d| d.add(e))
    }

    // ------------------------------------------------------------ prompts

    async fn ask(&self, kind: PromptKind, spec: PromptSpec) -> CmdResult<Reply> {
        AskFuture { ctx: self.clone(), pending: Some(Pending::new(kind, spec)) }.await
    }

    /// Specify a point.
    pub async fn point(&self, spec: PromptSpec) -> CmdResult<PointR> {
        match self.ask(PromptKind::Point, spec).await? {
            Reply::Pt { pt, .. } => Ok(PointR::Pt(pt)),
            Reply::Kw(k) => Ok(PointR::Kw(k)),
            Reply::Enter { pt, .. } => Ok(PointR::Enter(pt)),
            Reply::Num { value, rel, .. } => Ok(PointR::Num { value, rel }),
            _ => unexpected(),
        }
    }

    /// Second corner of a rectangle (shows the rubber-band rectangle from `base`).
    pub async fn corner(&self, base: Pt, mut spec: PromptSpec) -> CmdResult<PointR> {
        spec.base = Some(base);
        if spec.preview.is_none() {
            spec = spec.preview(move |_, p| vec![PreviewItem::Rect { a: base, b: p }]);
        }
        self.point(spec).await
    }

    /// Typed number.
    pub async fn number(&self, spec: PromptSpec) -> CmdResult<NumR> {
        match self.ask(PromptKind::Number, spec).await? {
            Reply::Num { value, .. } => Ok(NumR::Value(value)),
            Reply::Kw(k) => Ok(NumR::Kw(k)),
            Reply::Enter { value, .. } => Ok(NumR::Enter(value)),
            _ => unexpected(),
        }
    }

    /// Distance typed or measured by picking two points (or one with a base point).
    pub async fn distance(&self, spec: PromptSpec) -> CmdResult<MeasR> {
        self.measure(PromptKind::Distance, spec).await
    }

    /// Angle in degrees typed (returned in radians) or measured by picking points.
    pub async fn angle(&self, spec: PromptSpec) -> CmdResult<MeasR> {
        self.measure(PromptKind::Angle, spec).await
    }

    async fn measure(&self, kind: PromptKind, spec: PromptSpec) -> CmdResult<MeasR> {
        match self.ask(kind, spec).await? {
            Reply::Num { value, pt, .. } => Ok(MeasR::Value { value, pt }),
            Reply::Kw(k) => Ok(MeasR::Kw(k)),
            Reply::Enter { value, .. } => Ok(MeasR::Enter(value)),
            _ => unexpected(),
        }
    }

    pub async fn string(&self, spec: PromptSpec) -> CmdResult<StrR> {
        match self.ask(PromptKind::String, spec).await? {
            Reply::Text(t) => Ok(StrR { text: t, entered: false }),
            Reply::Enter { text, .. } => Ok(StrR { text: text.unwrap_or_default(), entered: true }),
            _ => unexpected(),
        }
    }

    pub async fn keyword(&self, spec: PromptSpec) -> CmdResult<KeyR> {
        match self.ask(PromptKind::Keyword, spec).await? {
            Reply::Kw(k) => Ok(KeyR::Kw(k)),
            Reply::Enter { kw, .. } => Ok(KeyR::Enter(kw)),
            _ => unexpected(),
        }
    }

    /// Pick one object.
    pub async fn entity(&self, spec: PromptSpec) -> CmdResult<EntR> {
        match self.ask(PromptKind::Entity, spec).await? {
            Reply::Entity { id, pt, shift } => Ok(EntR::Entity { id, pt, shift }),
            Reply::Kw(k) => Ok(EntR::Kw(k)),
            Reply::Enter { .. } => Ok(EntR::Enter),
            _ => unexpected(),
        }
    }

    /// Pick one 3D solid.
    pub async fn solid(&self, spec: PromptSpec) -> CmdResult<SolidR> {
        match self.ask(PromptKind::Solid, spec).await? {
            Reply::Solid { id, pt } => Ok(SolidR::Solid { id, pt }),
            Reply::Kw(k) => Ok(SolidR::Kw(k)),
            Reply::Enter { .. } => Ok(SolidR::Enter),
            _ => unexpected(),
        }
    }

    /// Single object with the pick box; `None` when Enter or a keyword answered instead.
    pub async fn pick_entity(&self, spec: PromptSpec) -> CmdResult<Option<u64>> {
        Ok(match self.entity(spec).await? {
            EntR::Entity { id, .. } => Some(id),
            _ => None,
        })
    }

    /// "Select objects:" loop. Honours pick-first selection.
    pub async fn select(&self, mut spec: PromptSpec) -> CmdResult<Selection> {
        if spec.message.is_empty() {
            spec.message = "Select objects".into();
        }
        if let Some(sel) = self.app(|a| a.pickfirst_selection(self.task, &spec)) {
            return Ok(sel);
        }
        match self.ask(PromptKind::Select, spec).await? {
            Reply::Selection(ids) => Ok(self.doc(|d| {
                let mut s = Selection::default();
                for id in ids {
                    if d.solid_by_id(id).is_some() {
                        s.solids.push(id);
                    } else {
                        s.ents.push(id);
                    }
                }
                s
            })),
            _ => unexpected(),
        }
    }

    /// Ask the UI for a modal dialog. Headless runs (no dialogs available) answer `None` immediately.
    pub async fn dialog(&self, req: DialogReq) -> CmdResult<Option<DialogReply>> {
        if !self.app(|a| a.ui.dialogs) {
            return Ok(None);
        }
        let mut p = Pending::new(PromptKind::Dialog, PromptSpec::new(""));
        p.dialog = Some(req);
        match (AskFuture { ctx: self.clone(), pending: Some(p) }).await? {
            Reply::Dialog(r) => Ok(Some(r)),
            _ => unexpected(),
        }
    }

    /// Run another command inline (the JS `c.rt.run(name)`); it follows the transparency rules.
    pub async fn sub(&self, name: &str, args: Args) {
        let started = self.app(|a| {
            let root = a.rt.tasks.get(&self.task).map(|t| t.root);
            let def = match a.registry.find(name) {
                Some(d) => d,
                None => {
                    a.log(&format!("Unknown command \"{name}\".  Press F1 for help."), "error");
                    return None;
                }
            };
            if !def.transparent {
                a.log("That command may not be used transparently.", "error");
                return None;
            }
            let id = a.begin_task(&def, true, false, root);
            Some((def, id))
        });
        let Some((def, id)) = started else { return };
        let child = Ctx::new(self.h.clone(), id);
        let res = (def.run)(child, args).await;
        self.app(|a| a.end_task(id, res));
    }
}

impl App {
    /// Objects already selected (pick-first) that satisfy the prompt, if any.
    pub(crate) fn pickfirst_selection(&mut self, task: TaskId, spec: &PromptSpec) -> Option<Selection> {
        let def_pick = self.rt.tasks.get(&task).map(|t| t.def.pickfirst).unwrap_or(true);
        if !(spec.pickfirst.unwrap_or(def_pick) && self.sysvars.int("PICKFIRST") != 0) {
            return None;
        }
        let ed = self.editor()?;
        if ed.sel.is_empty() {
            return None;
        }
        let ents: Vec<u64> = ed.selected().into_iter().filter(|e| self.selectable_by(e, spec.allow_off, spec.filter.as_ref())).map(|e| e.id).collect();
        let solids: Vec<u64> = if spec.solids { ed.selected_solids().into_iter().filter(|s| ed.doc.solid_visible(s)).map(|s| s.id).collect() } else { vec![] };
        if ents.is_empty() && solids.is_empty() {
            return None;
        }
        let n = ents.len() + solids.len();
        let temp = ents.iter().chain(solids.iter()).copied().collect();
        self.log(&format!("{n} found"), "info");
        self.editor_mut()?.temp_sel = temp;
        Some(Selection { ents, solids })
    }
}
