//! Palette, ribbon and workspace switches.
use super::ps;
use crate::app::Event;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;

/// The workspaces the host offers (the same names as the status-bar switcher).
pub const WORKSPACES: [&str; 3] = ["Drafting & Annotation", "3D Modeling", "CadForge Classic"];

fn palette(name: &'static str, show: Option<bool>) -> crate::registry::RunFn {
    run(move |c: Ctx, _a: Args| async move {
        c.app(|a| {
            a.emit(Event::Palette {
                name: name.to_string(),
                show,
            })
        });
        Ok(())
    })
}

async fn workspace(c: Ctx, args: Args) -> CmdResult<()> {
    if !c.app(|a| a.ui.dialogs) {
        c.log_error("WORKSPACE is not available in this environment.");
        return Ok(());
    }
    let typed = match args.str("name") {
        Some(n) => n.to_string(),
        None => {
            c.log(&format!("Workspaces: {}", WORKSPACES.join(", ")));
            c.string(ps("Enter workspace name to make current or [?]").default_str(WORKSPACES[0]))
                .await?
                .text
        }
    };
    let t = typed.trim().to_lowercase();
    if t == "?" {
        return Ok(());
    }
    match WORKSPACES.iter().find(|w| {
        w.to_lowercase().starts_with(&t) || (t.len() >= 2 && w.to_lowercase().contains(&t))
    }) {
        Some(w) => {
            c.app(|a| {
                a.emit(Event::UiAction {
                    name: "WORKSPACE".into(),
                    args: Args::none().with("name", ArgVal::Str(w.to_string())),
                })
            });
        }
        None => c.log_error(&format!("Workspace \"{}\" not found.", typed.trim())),
    }
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(
        def(
            "Utilities",
            "PROPERTIESCLOSE",
            palette("properties", Some(false)),
        )
        .title("Close Properties")
        .desc("Closes the Properties palette.")
        .transparent()
        .once(),
    );
    r.define(
        def(
            "Utilities",
            "TOOLPALETTESCLOSE",
            palette("tools", Some(false)),
        )
        .title("Close Tool Palettes")
        .desc("Closes the Tool Palettes window.")
        .transparent()
        .once(),
    );
    r.define(
        def("Utilities", "RIBBON", palette("ribbon", Some(true)))
            .title("Ribbon")
            .icon("ribbon")
            .desc("Opens the ribbon.")
            .transparent()
            .once(),
    );
    r.define(
        def("Utilities", "RIBBONCLOSE", palette("ribbon", Some(false)))
            .title("Close Ribbon")
            .desc("Closes the ribbon.")
            .transparent()
            .once(),
    );
    r.define(
        def("Utilities", "HIDEPALETTES", palette("hide-all", None))
            .title("Hide Palettes")
            .desc("Hides all currently displayed palettes (including the command line).")
            .transparent()
            .once(),
    );
    r.define(
        def("Utilities", "SHOWPALETTES", palette("show-all", None))
            .title("Restore Palettes")
            .desc("Restores the display of hidden palettes.")
            .transparent()
            .once(),
    );
    r.define(
        def("Blocks", "BLOCKPALETTE", palette("blocks", Some(true)))
            .title("Blocks Palette")
            .icon("block")
            .desc("Opens the palette that lists the blocks of the drawing; click one to insert it.")
            .transparent()
            .once(),
    );
    r.define(
        def("Utilities", "WORKSPACE", run(workspace))
            .title("Workspace")
            .icon("gear")
            .desc("Creates, modifies, and saves workspaces and makes a workspace current.")
            .transparent(),
    );
}
