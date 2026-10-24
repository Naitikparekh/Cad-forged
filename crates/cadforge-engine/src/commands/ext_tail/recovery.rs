//! DRAWINGRECOVERY: what autosave is doing for the open drawings.
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;

async fn drawingrecovery(c: Ctx, _a: Args) -> CmdResult<()> {
    let minutes = c.pref("SAVETIME");
    if minutes > 0.0 {
        c.log(&format!("Autosave is on: unsaved changes are kept every {} minute{} (system variable SAVETIME).", super::trim_num(minutes), if minutes == 1.0 { "" } else { "s" }));
    } else {
        c.log(
            "Autosave is off (SAVETIME is 0). Set SAVETIME to a number of minutes to turn it on.",
        );
    }
    let files = c.app(|a| a.autosave_status());
    if files.is_empty() {
        c.log("No open drawing has a recovery file yet.");
    } else {
        c.log("Drawings with a recovery file (removed when the drawing is saved or closed):");
        for (name, key) in files {
            c.log(&format!("  {name}  [{key}]"));
        }
    }
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(
        def("File", "DRAWINGRECOVERY", run(drawingrecovery))
            .title("Drawing Recovery")
            .icon("save")
            .desc("Shows whether autosave is on and which open drawings have a recovery file.")
            .once(),
    );
}
