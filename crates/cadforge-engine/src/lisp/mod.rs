//! A compact AutoLISP interpreter: enough of the language and of the AutoCAD functions (`command`, `entget`, `ssget`,
//! `getpoint`...) to run the routines people write for drawing automation.
//!
//! Evaluation is `async` because a routine may wait for the user (`getpoint`, `command` with `PAUSE`, `ssget`). Variables
//! are dynamically scoped, as in AutoLISP: parameters and locals of a `defun` live in a frame for the duration of the call.
use crate::ctx::Ctx;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

mod builtins;
mod cad;
mod entities;
pub mod print;
pub mod reader;
pub mod value;

pub use cad::{flush, out};

pub use value::*;

type LFut<'a> = Pin<Box<dyn Future<Output = LResult> + 'a>>;

/// Evaluation steps allowed for one top-level expression (a runaway `(while T)` ends with an error instead of freezing).
pub const DEFAULT_STEP_LIMIT: u64 = 40_000_000;
/// Continuation frames allowed (deep recursion); each costs a few dozen bytes.
const STACK_LIMIT: usize = 400_000;
/// Machines running inside one another (a function called by `mapcar` that itself calls `mapcar`...).
const NEST_LIMIT: usize = 60;

/// Machine registers.
enum St {
    Eval(Value),
    Ret(Value),
    Call(Value, Vec<Value>),
}

/// Continuations.
enum Fr {
    Args { head: Value, done: Vec<Value>, rest: VecDeque<Value> },
    Head(VecDeque<Value>),
    If(Option<Value>, Option<Value>),
    Setq(Rc<str>, VecDeque<Value>),
    Progn(VecDeque<Value>),
    /// `true` = and, `false` = or.
    Logic(bool, VecDeque<Value>),
    CondTest(VecDeque<Value>, Vec<Value>),
    WhileTest(Value, Rc<Vec<Value>>),
    WhileBody(Value, Rc<Vec<Value>>),
    RepeatCount(Rc<Vec<Value>>),
    RepeatBody(i64, Rc<Vec<Value>>),
    ForeachList(Rc<str>, Rc<Vec<Value>>),
    ForeachNext(Rc<str>, Rc<Vec<Value>>, VecDeque<Value>),
    PopFrame(usize),
}

/// State set by `initget` for the next `get...` call.
#[derive(Clone, Debug, Default)]
pub struct InitGet {
    pub bits: i64,
    pub keywords: Vec<String>,
}

#[derive(Default)]
pub struct Interp {
    globals: RefCell<HashMap<String, Value>>,
    /// Names bound by each active call, outermost first (so that a call's bindings can be undone).
    frames: RefCell<Vec<Vec<String>>>,
    /// Dynamic ("shallow") binding: for every name bound by an active call, its values with the innermost last.
    bindings: RefCell<HashMap<String, Vec<Value>>>,
    pub(crate) init_get: RefCell<InitGet>,
    steps: Cell<u64>,
    step_limit: Cell<u64>,
    nest: Cell<usize>,
    /// `c:` functions defined since the host last collected them.
    new_commands: RefCell<Vec<String>>,
    /// Text `princ` / `prin1` / `print` wrote last (so that the echo of a result is not doubled).
    pub(crate) last_printed: RefCell<Option<String>>,
    /// Output after the last newline, held back until the line is complete.
    pub(crate) out_buf: RefCell<String>,
}

/// Pops the variable frames a machine pushed, even when its evaluation is dropped half way (Escape at a prompt).
struct FrameGuard<'a> {
    it: &'a Interp,
    len: usize,
    nest: Option<usize>,
}

impl Drop for FrameGuard<'_> {
    fn drop(&mut self) {
        self.it.truncate_frames(self.len);
        if let Some(n) = self.nest {
            self.it.nest.set(n);
        }
    }
}

pub fn bad_arg(pred: &str, v: &Value) -> LErr {
    LErr::Msg(format!("bad argument type: {pred} {}", print::prin1_str(v)))
}

impl Interp {
    pub fn new() -> Interp {
        let it = Interp::default();
        it.step_limit.set(DEFAULT_STEP_LIMIT);
        it.globals.borrow_mut().insert("PI".into(), Value::Real(std::f64::consts::PI));
        it.globals.borrow_mut().insert("PAUSE".into(), Value::str("\\"));
        it
    }

    // ------------------------------------------------------------ variables

    pub fn lookup(&self, name: &str) -> Value {
        if let Some(v) = self.bindings.borrow().get(name).and_then(|st| st.last()) {
            return v.clone();
        }
        self.globals.borrow().get(name).cloned().unwrap_or(Value::Nil)
    }

    pub fn is_bound(&self, name: &str) -> bool {
        self.bindings.borrow().get(name).is_some_and(|st| !st.is_empty()) || self.globals.borrow().get(name).is_some_and(|v| v.is_true())
    }

    pub fn set(&self, name: &str, v: Value) {
        if let Some(slot) = self.bindings.borrow_mut().get_mut(name).and_then(|st| st.last_mut()) {
            *slot = v;
            return;
        }
        self.globals.borrow_mut().insert(name.to_string(), v);
    }

    /// Bind names for the duration of a call.
    fn push_frame(&self, vars: Vec<(String, Value)>) {
        let mut b = self.bindings.borrow_mut();
        let mut names = Vec::with_capacity(vars.len());
        for (n, v) in vars {
            b.entry(n.clone()).or_default().push(v);
            names.push(n);
        }
        self.frames.borrow_mut().push(names);
    }

    /// Undo the bindings of every call above the first `len`.
    fn truncate_frames(&self, len: usize) {
        let mut frames = self.frames.borrow_mut();
        let mut b = self.bindings.borrow_mut();
        while frames.len() > len {
            for n in frames.pop().unwrap_or_default() {
                if let Some(st) = b.get_mut(&n) {
                    st.pop();
                }
            }
        }
    }

    /// Set a global regardless of frames (used by hosts).
    pub fn set_global(&self, name: &str, v: Value) {
        self.globals.borrow_mut().insert(name.to_uppercase(), v);
    }

    /// Names of the `c:` functions defined since the last call.
    pub fn take_new_commands(&self) -> Vec<String> {
        std::mem::take(&mut self.new_commands.borrow_mut())
    }

    /// True when `name` is bound to a user function.
    pub fn is_function(&self, name: &str) -> bool {
        matches!(self.lookup(&name.to_uppercase()), Value::Fun(_))
    }

    /// Change how many evaluation steps one top-level expression may take.
    pub fn set_step_limit(&self, n: u64) {
        self.step_limit.set(n);
    }

    /// Start counting steps for a new top-level evaluation.
    pub fn begin(&self) {
        self.steps.set(0);
        self.nest.set(0);
    }

    // ------------------------------------------------------------ evaluation
    //
    // Evaluation is a small abstract machine with an explicit stack of continuations (`Fr`), so that deep Lisp
    // recursion consumes heap, not the native stack (a browser gives a program very little of that). The machine
    // is `async` only because `get...` / `command` may have to wait for the user.

    /// Evaluate `x`.
    pub fn eval<'a>(&'a self, c: &'a Ctx, x: &'a Value) -> LFut<'a> {
        Box::pin(self.run(c, St::Eval(x.clone())))
    }

    /// Evaluate every form of `body`, returning the last value.
    pub async fn progn(&self, c: &Ctx, body: &[Value]) -> LResult {
        let mut last = Value::Nil;
        for f in body {
            last = self.eval(c, f).await?;
        }
        Ok(last)
    }

    /// Evaluate every expression in `src`; returns the value of the last.
    pub async fn eval_source(&self, c: &Ctx, src: &str) -> LResult {
        let forms = reader::read_all(src).map_err(LErr::Msg)?;
        let mut last = Value::Nil;
        for f in &forms {
            last = self.eval(c, f).await?;
        }
        Ok(last)
    }

    /// Call a function value (or the function a symbol names) with evaluated arguments.
    pub fn apply<'a>(&'a self, c: &'a Ctx, f: &'a Value, args: Vec<Value>) -> LFut<'a> {
        Box::pin(self.run(c, St::Call(f.clone(), args)))
    }

    async fn run(&self, c: &Ctx, start: St) -> LResult {
        let nest = self.nest.get() + 1;
        if nest > NEST_LIMIT {
            return Err(LErr::Msg("stack overflow: functions nested too deeply".into()));
        }
        self.nest.set(nest);
        let _guard = FrameGuard { it: self, len: self.frames.borrow().len(), nest: Some(nest - 1) };
        let mut stack: Vec<Fr> = vec![];
        let mut st = start;
        loop {
            let n = self.steps.get() + 1;
            self.steps.set(n);
            if n > self.step_limit.get() {
                return Err(LErr::Msg("evaluation limit exceeded (runaway loop?)".into()));
            }
            if stack.len() > STACK_LIMIT {
                return Err(LErr::Msg("stack overflow: recursion too deep".into()));
            }
            st = match st {
                St::Eval(x) => self.eval_step(&mut stack, x)?,
                St::Ret(v) => match stack.pop() {
                    None => return Ok(v),
                    Some(fr) => self.resume(&mut stack, fr, v)?,
                },
                St::Call(f, args) => self.call_step(c, &mut stack, f, args).await?,
            };
        }
    }

    /// A sequence of forms: evaluate the first and continue with the rest (the last form is a tail call).
    fn seq(&self, stack: &mut Vec<Fr>, mut forms: VecDeque<Value>) -> St {
        match forms.pop_front() {
            None => St::Ret(Value::Nil),
            Some(first) => {
                if !forms.is_empty() {
                    stack.push(Fr::Progn(forms));
                }
                St::Eval(first)
            }
        }
    }

    fn eval_step(&self, stack: &mut Vec<Fr>, x: Value) -> LResult<St> {
        let cell = match x {
            Value::Sym(n) => return Ok(St::Ret(self.lookup(&n))),
            Value::Cons(cell) => cell,
            other => return Ok(St::Ret(other)),
        };
        let args = &cell.1;
        if let Value::Sym(name) = &cell.0 {
            let special = matches!(&**name, "QUOTE" | "FUNCTION" | "SETQ" | "DEFUN" | "LAMBDA" | "IF" | "COND" | "PROGN" | "WHILE" | "REPEAT" | "FOREACH" | "AND" | "OR");
            if special {
                let forms = args.to_vec().ok_or_else(|| LErr::Msg("malformed list".into()))?;
                return self.special(stack, name, forms);
            }
            // an ordinary call: evaluate the arguments left to right, then call
            let forms: VecDeque<Value> = args.to_vec().ok_or_else(|| LErr::Msg("malformed list".into()))?.into();
            return Ok(Self::start_args(stack, cell.0.clone(), forms));
        }
        if matches!(cell.0, Value::Cons(_)) {
            // ((lambda ...) args): the head is evaluated first
            let forms: VecDeque<Value> = args.to_vec().ok_or_else(|| LErr::Msg("malformed list".into()))?.into();
            stack.push(Fr::Head(forms));
            return Ok(St::Eval(cell.0.clone()));
        }
        Err(LErr::Msg(format!("bad function: {}", print::prin1_str(&cell.0))))
    }

    fn start_args(stack: &mut Vec<Fr>, head: Value, mut forms: VecDeque<Value>) -> St {
        match forms.pop_front() {
            None => St::Call(head, vec![]),
            Some(first) => {
                stack.push(Fr::Args { head, done: vec![], rest: forms });
                St::Eval(first)
            }
        }
    }

    fn special(&self, stack: &mut Vec<Fr>, name: &str, mut forms: Vec<Value>) -> LResult<St> {
        Ok(match name {
            "QUOTE" => St::Ret(forms.into_iter().next().unwrap_or(Value::Nil)),
            "FUNCTION" => match forms.first() {
                Some(f @ Value::Cons(_)) if matches!(f.car(), Some(Value::Sym(s)) if &*s == "LAMBDA") => St::Ret(self.make_lambda(None, &f.cdr().unwrap_or(Value::Nil))?),
                Some(f) => St::Ret(f.clone()),
                None => St::Ret(Value::Nil),
            },
            "LAMBDA" => St::Ret(self.make_lambda(None, &Value::list(forms))?),
            "DEFUN" => {
                let Some(Value::Sym(n)) = forms.first() else { return Err(LErr::Msg("bad argument type: symbolp".into())) };
                let f = self.make_lambda(Some(n.to_string()), &Value::list(forms[1..].to_vec()))?;
                self.globals.borrow_mut().insert(n.to_string(), f);
                if n.starts_with("C:") {
                    self.new_commands.borrow_mut().push(n.to_string());
                }
                St::Ret(Value::Sym(n.clone()))
            }
            "SETQ" => {
                let mut pairs: VecDeque<Value> = forms.into();
                match pairs.pop_front() {
                    None => St::Ret(Value::Nil),
                    Some(Value::Sym(n)) => match pairs.pop_front() {
                        None => {
                            self.set(&n, Value::Nil);
                            St::Ret(Value::Nil)
                        }
                        Some(e) => {
                            stack.push(Fr::Setq(n, pairs));
                            St::Eval(e)
                        }
                    },
                    Some(other) => return Err(bad_arg("symbolp", &other)),
                }
            }
            "IF" => {
                if forms.is_empty() {
                    return Err(LErr::Msg("too few arguments".into()));
                }
                let test = forms.remove(0);
                let mut it = forms.into_iter();
                stack.push(Fr::If(it.next(), it.next()));
                St::Eval(test)
            }
            "COND" => {
                let mut clauses: VecDeque<Value> = forms.into();
                self.next_clause(stack, &mut clauses)?
            }
            "PROGN" => self.seq(stack, forms.into()),
            "AND" | "OR" => {
                let mut rest: VecDeque<Value> = forms.into();
                match rest.pop_front() {
                    None => St::Ret(Value::bool(name == "AND")),
                    Some(first) => {
                        stack.push(Fr::Logic(name == "AND", rest));
                        St::Eval(first)
                    }
                }
            }
            "WHILE" => {
                if forms.is_empty() {
                    return Err(LErr::Msg("too few arguments".into()));
                }
                let test = forms.remove(0);
                let body: Rc<Vec<Value>> = Rc::new(forms);
                stack.push(Fr::WhileTest(test.clone(), body));
                St::Eval(test)
            }
            "REPEAT" => {
                if forms.is_empty() {
                    return Err(LErr::Msg("too few arguments".into()));
                }
                let n = forms.remove(0);
                stack.push(Fr::RepeatCount(Rc::new(forms)));
                St::Eval(n)
            }
            "FOREACH" => {
                let (Some(Value::Sym(var)), true) = (forms.first().cloned(), forms.len() >= 2) else { return Err(LErr::Msg("bad argument type: symbolp".into())) };
                let list_form = forms[1].clone();
                stack.push(Fr::ForeachList(var, Rc::new(forms[2..].to_vec())));
                St::Eval(list_form)
            }
            _ => unreachable!("only special-form names reach `special`"),
        })
    }

    /// Start the next COND clause (or finish with nil).
    fn next_clause(&self, stack: &mut Vec<Fr>, clauses: &mut VecDeque<Value>) -> LResult<St> {
        while let Some(clause) = clauses.pop_front() {
            let Some(items) = clause.to_vec() else { return Err(LErr::Msg("malformed COND clause".into())) };
            let Some(test) = items.first().cloned() else { continue };
            stack.push(Fr::CondTest(std::mem::take(clauses), items[1..].to_vec()));
            return Ok(St::Eval(test));
        }
        Ok(St::Ret(Value::Nil))
    }

    fn resume(&self, stack: &mut Vec<Fr>, fr: Fr, v: Value) -> LResult<St> {
        Ok(match fr {
            Fr::Args { head, mut done, mut rest } => {
                done.push(v);
                match rest.pop_front() {
                    None => St::Call(head, done),
                    Some(next) => {
                        stack.push(Fr::Args { head, done, rest });
                        St::Eval(next)
                    }
                }
            }
            Fr::Head(forms) => Self::start_args(stack, v, forms),
            Fr::If(then_, else_) => match (v.is_true(), then_, else_) {
                (true, Some(t), _) => St::Eval(t),
                (false, _, Some(e)) => St::Eval(e),
                _ => St::Ret(Value::Nil),
            },
            Fr::Setq(name, mut rest) => {
                self.set(&name, v.clone());
                match rest.pop_front() {
                    None => St::Ret(v),
                    Some(Value::Sym(n)) => match rest.pop_front() {
                        None => {
                            self.set(&n, Value::Nil);
                            St::Ret(Value::Nil)
                        }
                        Some(e) => {
                            stack.push(Fr::Setq(n, rest));
                            St::Eval(e)
                        }
                    },
                    Some(other) => return Err(bad_arg("symbolp", &other)),
                }
            }
            Fr::Progn(rest) => self.seq(stack, rest),
            Fr::Logic(is_and, mut rest) => {
                if is_and == v.is_nil() {
                    // `and` stops at the first nil, `or` at the first non-nil
                    St::Ret(v)
                } else {
                    match rest.pop_front() {
                        None => St::Ret(v),
                        Some(next) => {
                            stack.push(Fr::Logic(is_and, rest));
                            St::Eval(next)
                        }
                    }
                }
            }
            Fr::CondTest(mut clauses, body) => {
                if v.is_true() {
                    if body.is_empty() {
                        St::Ret(v)
                    } else {
                        self.seq(stack, body.into())
                    }
                } else {
                    self.next_clause(stack, &mut clauses)?
                }
            }
            Fr::WhileTest(test, body) => {
                if v.is_nil() {
                    St::Ret(Value::Nil)
                } else {
                    stack.push(Fr::WhileBody(test, body.clone()));
                    self.seq(stack, body.iter().cloned().collect())
                }
            }
            Fr::WhileBody(test, body) => {
                stack.push(Fr::WhileTest(test.clone(), body));
                St::Eval(test)
            }
            Fr::RepeatCount(body) => {
                let n = v.as_i64().ok_or_else(|| bad_arg("numberp", &v))?;
                if n <= 0 {
                    St::Ret(Value::Nil)
                } else {
                    stack.push(Fr::RepeatBody(n - 1, body.clone()));
                    self.seq(stack, body.iter().cloned().collect())
                }
            }
            Fr::RepeatBody(left, body) => {
                if left <= 0 {
                    St::Ret(v)
                } else {
                    stack.push(Fr::RepeatBody(left - 1, body.clone()));
                    self.seq(stack, body.iter().cloned().collect())
                }
            }
            Fr::ForeachList(var, body) => {
                let items: VecDeque<Value> = v.to_vec().ok_or_else(|| bad_arg("listp", &v))?.into();
                let len = self.frames.borrow().len();
                self.push_frame(vec![(var.to_string(), Value::Nil)]);
                stack.push(Fr::PopFrame(len));
                self.foreach_next(stack, var, body, items, Value::Nil)
            }
            Fr::ForeachNext(var, body, items) => self.foreach_next(stack, var, body, items, v),
            Fr::PopFrame(len) => {
                self.truncate_frames(len);
                St::Ret(v)
            }
        })
    }

    fn foreach_next(&self, stack: &mut Vec<Fr>, var: Rc<str>, body: Rc<Vec<Value>>, mut items: VecDeque<Value>, last: Value) -> St {
        match items.pop_front() {
            None => St::Ret(last),
            Some(it) => {
                self.set(&var, it);
                stack.push(Fr::ForeachNext(var, body.clone(), items));
                self.seq(stack, body.iter().cloned().collect())
            }
        }
    }

    async fn call_step(&self, c: &Ctx, stack: &mut Vec<Fr>, f: Value, args: Vec<Value>) -> LResult<St> {
        match f {
            Value::Fun(l) => {
                if args.len() > l.params.len() {
                    return Err(LErr::Msg("too many arguments".into()));
                }
                let mut vars: Vec<(String, Value)> = Vec::with_capacity(l.params.len() + l.locals.len());
                for (i, p) in l.params.iter().enumerate() {
                    vars.push((p.clone(), args.get(i).cloned().unwrap_or(Value::Nil)));
                }
                for p in &l.locals {
                    vars.push((p.clone(), Value::Nil));
                }
                let len = self.frames.borrow().len();
                self.push_frame(vars);
                stack.push(Fr::PopFrame(len));
                Ok(self.seq(stack, l.body.iter().cloned().collect()))
            }
            Value::Sym(name) => {
                match self.lookup(&name) {
                    f @ Value::Fun(_) => return Box::pin(self.call_step(c, stack, f, args)).await,
                    // a variable holding a function name: `(defun twice (f v) (f (f v)))` called with '1+
                    f @ Value::Sym(_) if !matches!(&f, Value::Sym(other) if *other == name) => return Box::pin(self.call_step(c, stack, f, args)).await,
                    _ => {}
                }
                if let Some(r) = self.higher_order(c, &name, &args).await {
                    return r.map(St::Ret);
                }
                if let Some(r) = cad::call(self, c, &name, &args).await {
                    return r.map(St::Ret);
                }
                if let Some(r) = entities::call(self, c, &name, &args).await {
                    return r.map(St::Ret);
                }
                if let Some(r) = builtins::call(self, &name, &args) {
                    return r.map(St::Ret);
                }
                Err(LErr::Msg(format!("no function definition: {name}")))
            }
            Value::Cons(_) if matches!(f.car(), Some(Value::Sym(s)) if &*s == "LAMBDA") => {
                let made = self.make_lambda(None, &f.cdr().unwrap_or(Value::Nil))?;
                Box::pin(self.call_step(c, stack, made, args)).await
            }
            other => Err(LErr::Msg(format!("bad function: {}", print::prin1_str(&other)))),
        }
    }

    /// `(params / locals) body...` → a function value.
    fn make_lambda(&self, name: Option<String>, spec: &Value) -> LResult {
        let items = spec.to_vec().ok_or_else(|| LErr::Msg("malformed function definition".into()))?;
        let Some(plist) = items.first() else { return Err(LErr::Msg("too few arguments".into())) };
        let plist = plist.to_vec().ok_or_else(|| LErr::Msg("malformed parameter list".into()))?;
        let (mut params, mut locals, mut after_slash) = (vec![], vec![], false);
        for p in &plist {
            match p {
                Value::Sym(s) if &**s == "/" => after_slash = true,
                Value::Sym(s) => {
                    if after_slash {
                        locals.push(s.to_string())
                    } else {
                        params.push(s.to_string())
                    }
                }
                other => return Err(bad_arg("symbolp", other)),
            }
        }
        Ok(Value::Fun(Rc::new(Lambda { name, params, locals, body: items[1..].to_vec() })))
    }

    /// Functions that call back into the interpreter.
    async fn higher_order(&self, c: &Ctx, name: &str, a: &[Value]) -> Option<LResult> {
        let need = |n: usize| if a.len() < n { Some(Err(LErr::Msg("too few arguments".into()))) } else { None };
        match name {
            "MAPCAR" | "MAPCAN" => {
                if let Some(e) = need(2) {
                    return Some(e);
                }
                let mut lists = vec![];
                for l in &a[1..] {
                    match l.to_vec() {
                        Some(v) => lists.push(v),
                        None => return Some(Err(bad_arg("listp", l))),
                    }
                }
                let n = lists.iter().map(Vec::len).min().unwrap_or(0);
                let mut out = vec![];
                for i in 0..n {
                    let args: Vec<Value> = lists.iter().map(|l| l[i].clone()).collect();
                    match self.apply(c, &a[0], args).await {
                        Ok(v) => out.push(v),
                        Err(e) => return Some(Err(e)),
                    }
                }
                if name == "MAPCAN" {
                    let mut flat = vec![];
                    for v in out {
                        match v.to_vec() {
                            Some(mut x) => flat.append(&mut x),
                            None if v.is_nil() => {}
                            None => return Some(Err(bad_arg("listp", &v))),
                        }
                    }
                    return Some(Ok(Value::list(flat)));
                }
                Some(Ok(Value::list(out)))
            }
            "APPLY" => {
                if let Some(e) = need(2) {
                    return Some(e);
                }
                let args = match a[1].to_vec() {
                    Some(v) => v,
                    None => return Some(Err(bad_arg("listp", &a[1]))),
                };
                Some(self.apply(c, &a[0], args).await)
            }
            "EVAL" => {
                if let Some(e) = need(1) {
                    return Some(e);
                }
                Some(self.eval(c, &a[0]).await)
            }
            "VL-REMOVE-IF" | "VL-REMOVE-IF-NOT" | "VL-MEMBER-IF" | "VL-MEMBER-IF-NOT" | "VL-SOME" | "VL-EVERY" | "VL-POSITION-IF" => {
                if let Some(e) = need(2) {
                    return Some(e);
                }
                let items = match a[1].to_vec() {
                    Some(v) => v,
                    None => return Some(Err(bad_arg("listp", &a[1]))),
                };
                let mut kept = vec![];
                for (i, it) in items.iter().enumerate() {
                    let hit = match self.apply(c, &a[0], vec![it.clone()]).await {
                        Ok(v) => v,
                        Err(e) => return Some(Err(e)),
                    };
                    match name {
                        "VL-REMOVE-IF" if hit.is_nil() => kept.push(it.clone()),
                        "VL-REMOVE-IF-NOT" if hit.is_true() => kept.push(it.clone()),
                        "VL-SOME" if hit.is_true() => return Some(Ok(hit)),
                        "VL-EVERY" if hit.is_nil() => return Some(Ok(Value::Nil)),
                        "VL-MEMBER-IF" if hit.is_true() => return Some(Ok(Value::list(items[i..].to_vec()))),
                        "VL-MEMBER-IF-NOT" if hit.is_nil() => return Some(Ok(Value::list(items[i..].to_vec()))),
                        "VL-POSITION-IF" if hit.is_true() => return Some(Ok(Value::Int(i as i64))),
                        _ => {}
                    }
                }
                Some(Ok(match name {
                    "VL-REMOVE-IF" | "VL-REMOVE-IF-NOT" => Value::list(kept),
                    "VL-EVERY" => Value::T,
                    _ => Value::Nil,
                }))
            }
            "VL-SORT" | "VL-SORT-I" => {
                if let Some(e) = need(2) {
                    return Some(e);
                }
                let items = match a[0].to_vec() {
                    Some(v) => v,
                    None => return Some(Err(bad_arg("listp", &a[0]))),
                };
                // insertion sort with the user's predicate (a stable order for equal items)
                let mut idx: Vec<usize> = vec![];
                for i in 0..items.len() {
                    let mut pos = idx.len();
                    for (k, j) in idx.iter().enumerate() {
                        match self.apply(c, &a[1], vec![items[i].clone(), items[*j].clone()]).await {
                            Ok(v) if v.is_true() => {
                                pos = k;
                                break;
                            }
                            Ok(_) => {}
                            Err(e) => return Some(Err(e)),
                        }
                    }
                    idx.insert(pos, i);
                }
                Some(Ok(if name == "VL-SORT" { Value::list(idx.iter().map(|i| items[*i].clone()).collect()) } else { Value::list(idx.iter().map(|i| Value::Int(*i as i64)).collect()) }))
            }
            "VL-CATCH-ALL-APPLY" => {
                if let Some(e) = need(2) {
                    return Some(e);
                }
                let args = a[1].to_vec().unwrap_or_default();
                match self.apply(c, &a[0], args).await {
                    Ok(v) => Some(Ok(v)),
                    Err(LErr::Msg(m)) => Some(Ok(Value::cons(Value::sym("VL-ERROR"), Value::str(&m)))),
                    Err(LErr::Cancel) => Some(Ok(Value::cons(Value::sym("VL-ERROR"), Value::str("Function cancelled")))),
                    Err(e) => Some(Err(e)),
                }
            }
            "VL-CATCH-ALL-ERROR-P" => Some(Ok(Value::bool(matches!(a.first(), Some(Value::Cons(c)) if matches!(&c.0, Value::Sym(s) if &**s == "VL-ERROR"))))),
            "VL-CATCH-ALL-ERROR-MESSAGE" => Some(Ok(match a.first() {
                Some(Value::Cons(c)) if matches!(&c.0, Value::Sym(s) if &**s == "VL-ERROR") => c.1.clone(),
                _ => Value::Nil,
            })),
            _ => None,
        }
    }
}
