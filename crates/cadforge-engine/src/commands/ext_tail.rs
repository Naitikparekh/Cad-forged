//! Long-tail AutoCAD commands (parity batch: ext_tail): TRACE, HATCHGENERATEBOUNDARY, SETBYLAYER, TXT2MTXT,
//! LISTSELECT, 3D navigation synonyms, VIEWGO, palette / ribbon switches and the CSV extracts (TABLEEXPORT, ATTEXT,
//! DATAEXTRACTION).
//!
//! Commands that produce a file (the extracts) emit [`crate::app::Event::SaveFile`]; the host downloads it (browser) or
//! writes it next to the drawing (native).
use crate::runtime::PromptSpec;

pub(crate) mod archive;
mod data;
mod defs;
mod draw;
mod images;
mod lisp_cmds;
mod nav;
mod props;
mod recovery;
mod small;
mod ui;
mod visibility;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn trim_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

pub fn register(r: &mut crate::registry::Registry) {
    draw::register(r);
    images::register(r);
    lisp_cmds::register(r);
    props::register(r);
    visibility::register(r);
    defs::register(r);
    archive::register(r);
    recovery::register(r);
    small::register(r);
    nav::register(r);
    ui::register(r);
    data::register(r);
}
