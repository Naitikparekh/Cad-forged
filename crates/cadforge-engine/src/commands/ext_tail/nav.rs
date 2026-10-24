//! 3D navigation synonyms and VIEWGO.
use super::ps;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;

/// VIEWGO: restore a named view.
async fn viewgo(c: Ctx, args: Args) -> CmdResult<()> {
    let names: Vec<String> = c.doc(|d| d.views.keys().cloned().collect());
    if names.is_empty() {
        c.log_error("There are no named views in this drawing.");
        return Ok(());
    }
    let typed = match args.str("name") {
        Some(n) => n.to_string(),
        None => c.string(ps("Enter view name to restore").default_str(&names[0])).await?.text,
    };
    let Some(n) = names.iter().find(|n| n.eq_ignore_ascii_case(typed.trim())) else {
        c.log_error(&format!("View {} not found.", typed.trim()));
        return Ok(());
    };
    let v = c.doc(|d| d.views.get(n).copied());
    if let Some(v) = v {
        c.view(|vw| {
            vw.push();
            vw.set_state(&v.to_state());
        });
        c.emit();
    }
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(def("View", "VIEWGO", run(viewgo)).title("Restore Named View").icon("view").desc("Restores a named view.").transparent());
    // the realtime pan, zoom and free orbit already work on the current (2D or 3D) view
    r.alias("3DPAN", "PAN");
    r.alias("3DZOOM", "ZOOM");
    r.alias("3DFORBIT", "3DORBIT");
}
