//! Additional AutoCAD commands (parity batch: ext_system): drawing limits, layer tools, visual styles and
//! view switches, clipboard variants, drawing reports and file / utility commands.
//!
//! LAYERP, TIME and CELTSCALE need to know what happened between commands. There is no engine hook for that, so
//! [`track_commands`] wraps every registered command with a call to [`observe`], which updates a [`Tracker`] owned
//! by the wrapped commands (one per registry, so one per [`crate::app::App`]).
//!
//! Commands that need the host UI emit [`Event::UiAction`] like the commands in `system.rs`. Besides the actions the
//! host already handles (`QUIT`, `QSAVE`, `PLOT`, `EXPORTPDF`, `SCRIPT`, `OPTIONS`; PREVIEW adds a `preview` flag to `PLOT`)
//! it must handle `CLEANSCREENOFF`, `TEXTSCR`, `GRAPHSCR` and `COPYHIST` (argument `lines`: the command history).
use crate::app::Event;
use crate::ctx::Ctx;
use crate::registry::{CommandDef, Registry};
use crate::runtime::*;
use cadforge_core::style::Layer;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

mod clipboard;
mod drafting;
mod files;
pub use files::{with_ext, SaveFormat};
mod layers;
mod report;
mod view;
mod vplayer;

/// How many earlier layer tables LAYERP can step back through.
const LAYER_HISTORY: usize = 50;

/// What the tracker remembers about one open drawing between commands.
struct DocTrack {
    /// `Drawing::rev` when the drawing was last looked at.
    rev: u64,
    layers: Vec<Layer>,
    /// Layer tables from before earlier changes, newest last (LAYERP).
    history: Vec<Vec<Layer>>,
    /// Session clock (milliseconds) when the drawing was first seen / last seen modified (TIME).
    opened: f64,
    updated: Option<f64>,
    timer_on: bool,
    timer_ms: f64,
    timer_since: f64,
}

/// Per-drawing bookkeeping keyed by [`crate::editor::Editor::id`].
#[derive(Default)]
struct Tracker {
    docs: HashMap<u64, DocTrack>,
}

type Shared = Rc<RefCell<Tracker>>;

/// Look at the active drawing before a command starts; remember its layer table whenever the drawing has changed.
fn observe(st: &Shared, c: &Ctx) {
    let Some((id, rev, now)) = c.app(|a| a.editor().map(|e| (e.id, e.doc.rev, a.now))) else {
        return;
    };
    if st.borrow().docs.get(&id).is_some_and(|d| d.rev == rev) {
        return;
    }
    let layers = c.doc(|d| d.layers.clone());
    let track_layers = c.pref("LAYERPMODE") != 0.0;
    let mut t = st.borrow_mut();
    match t.docs.get_mut(&id) {
        None => {
            t.docs.insert(
                id,
                DocTrack {
                    rev,
                    layers,
                    history: vec![],
                    opened: now,
                    updated: None,
                    timer_on: true,
                    timer_ms: 0.0,
                    timer_since: now,
                },
            );
        }
        Some(d) => {
            if d.layers != layers {
                if track_layers {
                    d.history.push(std::mem::replace(&mut d.layers, layers));
                } else {
                    // LAYERPMODE is off: keep no history, only follow the current table
                    d.layers = layers;
                }
                if d.history.len() > LAYER_HISTORY {
                    d.history.remove(0);
                }
            }
            d.rev = rev;
            d.updated = Some(now);
        }
    }
}

/// CELTSCALE applies to objects created by drawing commands (copies and offsets keep their source's scale).
fn apply_celtscale(c: &Ctx, first_new_id: u64) {
    let scale = c.doc(|d| d.settings.celtscale);
    if scale == 1.0 {
        return;
    }
    c.doc_mut(|d| {
        let ids: Vec<u64> = d
            .entities()
            .iter()
            .filter(|e| e.id >= first_new_id && e.lts.is_none())
            .map(|e| e.id)
            .collect();
        if !ids.is_empty() {
            d.change(|d| {
                for id in ids {
                    if let Some(e) = d.by_id_mut(id) {
                        e.lts = Some(scale);
                    }
                }
            });
        }
    });
}

/// Wrap every command: observe the drawing first, and give new objects of drawing commands their CELTSCALE.
fn track_commands(r: &mut Registry, st: &Shared) {
    let defs: Vec<CommandDef> = r.defs().map(|d| (**d).clone()).collect();
    let aliases = r.all_aliases();
    for mut d in defs {
        let (orig, st, draws) = (d.run.clone(), st.clone(), d.group == "Draw");
        d.run = Rc::new(move |c, a| {
            let (orig, st) = (orig.clone(), st.clone());
            Box::pin(async move {
                observe(&st, &c);
                let first_new = draws.then(|| c.doc(|d| d.next_id));
                let res = orig(c.clone(), a).await;
                if let Some(id) = first_new {
                    apply_celtscale(&c, id);
                }
                res
            })
        });
        r.define(d);
    }
    // defining a command registers its aliases again: put the table back as it was (the last definition wins)
    for (a, n) in aliases {
        r.alias(&a, &n);
    }
}

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

/// Hand a command over to the host UI; headless runs report that it is unavailable.
fn ui_action(c: &Ctx, command: &str, action: &str, args: Args) {
    if c.app(|a| a.ui.dialogs) {
        c.app(|a| {
            a.emit(Event::UiAction {
                name: action.to_string(),
                args,
            })
        });
    } else {
        c.log_error(&format!("{command} is not available in this environment."));
    }
}

/// A Yes/No question in AutoCAD's wording (`Question? [Yes/No] <No>:`); anything starting with Y is a yes.
async fn confirm(c: &Ctx, question: &str) -> CmdResult<bool> {
    let r = c
        .string(ps(&format!("{question} [Yes/No]")).default_str("No"))
        .await?;
    Ok(r.text.to_lowercase().starts_with('y'))
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

pub fn register(r: &mut Registry) {
    let st = Shared::default();
    layers::register(r, &st);
    vplayer::register(r);
    view::register(r);
    drafting::register(r);
    clipboard::register(r);
    report::register(r, &st);
    files::register(r);
    r.alias("DISTANCE", "DIST");
    track_commands(r, &st);
}
