//! Running Lisp: expressions typed at the command line, `(defun c:name ...)` commands, VLISP and APPLOAD for `.lsp` files.
use super::ps;
use crate::app::Event;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::lisp::{self, print, reader, Interp, LErr, Value};
use crate::registry::{CommandDef, Registry};
use crate::runtime::*;
use std::rc::Rc;

/// What the command line shows for the value of a top-level expression.
fn echo(it: &Interp, c: &Ctx, v: &Value) {
    if matches!(v, Value::Sym(s) if s.is_empty()) {
        return;
    }
    let shown = print::prin1_str(v);
    // `(princ "x")` already wrote x; do not print it twice
    let printed = it.last_printed.borrow_mut().take();
    if matches!(v, Value::Str(s) if printed.as_deref() == Some(&**s)) {
        return;
    }
    lisp::flush(it, c);
    c.log(&shown);
}

/// Report an error the way AutoLISP does, after giving a user `*error*` function the chance to handle it.
async fn report(it: &Interp, c: &Ctx, msg: &str) {
    lisp::flush(it, c);
    let handler = it.lookup("*ERROR*");
    if matches!(handler, Value::Fun(_)) {
        if let Err(e) = it.apply(c, &handler, vec![Value::str(msg)]).await {
            if let LErr::Msg(m) = e {
                c.log_error(&format!("; error in *error*: {m}"));
            }
        }
    } else {
        c.log_error(&format!("; error: {msg}"));
    }
}

/// Make the `c:` functions defined so far typeable commands.
fn register_new_commands(it: &Rc<Interp>, c: &Ctx) {
    for fname in it.take_new_commands() {
        let cmd = fname.trim_start_matches("C:").to_string();
        if cmd.is_empty() {
            continue;
        }
        let (it2, f2) = (it.clone(), fname.clone());
        let d = CommandDef::new(
            &cmd,
            run(move |c, _a| {
                let (it, f) = (it2.clone(), f2.clone());
                async move { call_user_command(&c, &it, &f).await }
            }),
        )
        .group("Lisp")
        .title(&cmd)
        .desc("A command defined by an AutoLISP routine.");
        c.app(|a| a.registry.define_user(d));
    }
}

async fn call_user_command(c: &Ctx, it: &Rc<Interp>, fname: &str) -> CmdResult<()> {
    it.begin();
    let f = it.lookup(fname);
    let r = it.apply(c, &f, vec![]).await;
    finish(it, c, r, false).await
}

/// Common ending of a Lisp evaluation: errors, echo and output buffer.
async fn finish(it: &Rc<Interp>, c: &Ctx, r: Result<Value, LErr>, show: bool) -> CmdResult<()> {
    register_new_commands(it, c);
    match r {
        Ok(v) => {
            if show {
                echo(it, c, &v);
            }
            lisp::flush(it, c);
            Ok(())
        }
        Err(LErr::Msg(m)) => {
            report(it, c, &m).await;
            Ok(())
        }
        Err(LErr::Cancel) => {
            report(it, c, "Function cancelled").await;
            Err(CmdError::Cancel)
        }
        Err(LErr::Exit) => {
            lisp::flush(it, c);
            Ok(())
        }
    }
}

/// `_LISP`: evaluate the expressions of `source`, asking for continuation lines while parentheses are open.
async fn eval_command(c: Ctx, a: Args) -> CmdResult<()> {
    let it = c.app(|app| app.lisp.clone());
    let mut src = a.str("source").unwrap_or("").to_string();
    let mut depth = 1;
    while reader::unbalanced(&src) {
        let more = c.string(ps(&format!("{depth}>")).allow_enter().raw()).await?;
        src.push('\n');
        src.push_str(&more.text);
        depth += 1;
    }
    // `!name` prints the value of a variable
    if let Some(var) = src.trim().strip_prefix('!') {
        let v = it.lookup(&var.trim().to_uppercase());
        c.log(&print::prin1_str(&v));
        return Ok(());
    }
    it.begin();
    let forms = match reader::read_all(&src) {
        Ok(f) => f,
        Err(e) => {
            c.log_error(&format!("; error: {e}"));
            return Ok(());
        }
    };
    let show = a.flag("echo") || src.trim_start().starts_with('(');
    let mut last = Ok(Value::Nil);
    for f in &forms {
        last = it.eval(&c, f).await;
        register_new_commands(&it, &c);
        if last.is_err() {
            break;
        }
    }
    finish(&it, &c, last, show).await
}

async fn vlisp(c: Ctx, _a: Args) -> CmdResult<()> {
    if c.app(|a| a.ui.dialogs) {
        c.app(|a| a.emit(Event::UiAction { name: "VLISP".into(), args: Args::none() }));
    } else {
        c.log("Type AutoLISP expressions at the command line, e.g. (+ 1 2).");
    }
    Ok(())
}

/// ACTRECORD: from now on, remember what the commands I run are given at their prompts.
async fn actrecord(c: Ctx, _a: Args) -> CmdResult<()> {
    if c.app(|a| a.recording.is_some()) {
        c.log_error("The Action Recorder is already recording. Use ACTSTOP to finish.");
        return Ok(());
    }
    c.app(|a| a.recording = Some(vec![]));
    c.log("Recording. Run commands as usual, then use ACTSTOP to save the macro.");
    Ok(())
}

/// ACTSTOP: finish recording and hand the macro over as a script (.scr) that SCRIPT can replay.
async fn actstop(c: Ctx, a: Args) -> CmdResult<()> {
    let Some(lines) = c.app(|app| app.recording.take()) else {
        c.log_error("The Action Recorder is not recording. Start it with ACTRECORD.");
        return Ok(());
    };
    if lines.is_empty() {
        c.log("Nothing was recorded.");
        return Ok(());
    }
    let name = match a.str("name") {
        Some(n) => n.to_string(),
        None => c.string(ps("Enter a name for the action macro").default_str("Macro1")).await?.text,
    };
    let name = name.trim().trim_end_matches(".scr").to_string();
    let name = if name.is_empty() { "Macro1".to_string() } else { name };
    let n = lines.iter().filter(|l| !l.is_empty()).count();
    let mut text = lines.join("\r\n");
    text.push_str("\r\n");
    c.app(|app| {
        app.emit(Event::SaveFile { name: format!("{name}.scr"), data: text.into_bytes(), mime: "text/plain".into() });
        app.log(&format!("Recorded {n} step{} to {name}.scr. Replay it with SCRIPT.", if n == 1 { "" } else { "s" }), "info");
    });
    Ok(())
}

/// `(load "file")` cannot read the file system of a browser; routines are loaded with APPLOAD.
pub(crate) fn register(r: &mut Registry) {
    r.define(def("Utilities", "ACTRECORD", run(actrecord)).aliases(&["RECORD"]).title("Record Action Macro").icon("script").desc("Starts the Action Recorder: records the commands you run so they can be replayed.").once());
    r.define(def("Utilities", "ACTSTOP", run(actstop)).title("Stop Recording").icon("script").desc("Stops the Action Recorder and saves the macro as a script file.").once());
    r.define(def("Utilities", "_LISP", run(eval_command)).title("Evaluate AutoLISP").desc("Evaluates AutoLISP expressions.").silent().once());
    r.define(def("Utilities", "VLISP", run(vlisp)).aliases(&["VLIDE"]).title("Visual LISP Console").icon("script").desc("Opens the AutoLISP console where routines can be typed, run and loaded.").once());
}
