//! Additional annotation commands (parity batch): DDEDIT, FIND, SPELL, FIELD, SCALETEXT, JUSTIFYTEXT, TOLERANCE,
//! MLEADERSTYLE, MLEADEREDIT, TABLE, TABLEEDIT, TABLESTYLE and the extra dimension commands (DIMARC, QDIM, DIMJOGGED,
//! DIMORDINATE, DIMBREAK, DIMCENTER, DIMSPACE, DIMEDIT, DIMTEDIT, DIMINSPECT).
use crate::ctx::Ctx;
use crate::registry::Registry;
use crate::runtime::PromptSpec;
use cadforge_core::entities::{Entity, Kind};

mod dims;
mod leaders;
mod spell;
mod tables;
mod textcmds;
mod tolerance;

pub fn register(r: &mut Registry) {
    dims::register(r);
    leaders::register(r);
    tables::register(r);
    textcmds::register(r);
    tolerance::register(r);
}

fn ps(message: &str) -> PromptSpec {
    PromptSpec::new(message)
}

/// A fresh anonymous group id, so that the parts of one annotation select together.
fn group_id(c: &Ctx) -> String {
    let serial = c.app(|a| {
        a.mem.group_serial += 1;
        a.mem.group_serial
    });
    format!("*A{serial}_{}", c.doc(|d| d.next_id))
}

/// Call `f` on every piece of user-visible text of an annotation object: text contents, dimension text overrides,
/// leader contents, table cells and block attribute values.
fn visit_strings(e: &mut Entity, f: &mut dyn FnMut(&mut String)) {
    match &mut e.kind {
        Kind::Text(t) => f(&mut t.s),
        Kind::Dim(d) => {
            if let Some(s) = d.txt.as_mut().filter(|s| !s.is_empty()) {
                f(s);
            }
        }
        Kind::Leader { s, .. } => f(s),
        Kind::Table(t) => {
            for s in t.cells.iter_mut().flatten() {
                f(s);
            }
        }
        Kind::Insert(i) => i.attribs.iter_mut().for_each(|a| f(&mut a.t.s)),
        _ => {}
    }
}

/// The user-visible strings of an annotation object (see [`visit_strings`]).
fn strings_of(e: &Entity) -> Vec<String> {
    let mut out = vec![];
    visit_strings(&mut e.clone(), &mut |s| out.push(s.clone()));
    out
}
