//! Long-tail AutoCAD commands (parity batch: ext_tail): TRACE, HATCHGENERATEBOUNDARY, SETBYLAYER, TXT2MTXT,
//! LISTSELECT, 3D navigation synonyms, VIEWGO, palette / ribbon switches and the CSV extracts (TABLEEXPORT, ATTEXT,
//! DATAEXTRACTION).
//!
//! Commands that produce a file (the extracts) emit [`crate::app::Event::SaveFile`]; the host downloads it (browser) or
//! writes it next to the drawing (native).
use crate::runtime::PromptSpec;

mod data;
mod draw;
mod images;
mod lisp_cmds;
mod nav;
mod props;
mod ui;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

pub fn register(r: &mut crate::registry::Registry) {
    draw::register(r);
    images::register(r);
    lisp_cmds::register(r);
    props::register(r);
    nav::register(r);
    ui::register(r);
    data::register(r);
}
