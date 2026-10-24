//! Drafting settings: LIMITS and CELTSCALE.
use super::ps;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::geom::pt;

async fn limits(c: Ctx, _a: Args) -> CmdResult<()> {
    if !c.doc(|d| d.is_model()) {
        c.log_error("LIMITS is available in model space only.");
        return Ok(());
    }
    let cur = c.doc(|d| d.settings.limits);
    c.log("Reset Model space limits:");
    let r = c
        .point(
            ps("Specify lower left corner or")
                .options(&["ON", "OFF"])
                .default_pt(pt(cur.min_x, cur.min_y)),
        )
        .await?;
    if let Some(k) = r.kw() {
        let on = k == "ON";
        c.doc_mut(|d| d.change(|d| d.settings.limits.on = on));
        c.log(&format!("Limits checking is {}.", if on { "on (recorded in the drawing; points outside the limits are not rejected in this version)" } else { "off" }));
        return Ok(());
    }
    let Some(ll) = r.pt() else { return Ok(()) };
    let Some(ur) = c
        .corner(
            ll,
            ps("Specify upper right corner").default_pt(pt(cur.max_x, cur.max_y)),
        )
        .await?
        .pt()
    else {
        return Ok(());
    };
    if ur.x <= ll.x || ur.y <= ll.y {
        c.log_error("Invalid limits: the upper right corner must be above and to the right of the lower left corner.");
        return Ok(());
    }
    c.doc_mut(|d| {
        d.change(|d| {
            let l = &mut d.settings.limits;
            (l.min_x, l.min_y, l.max_x, l.max_y) = (ll.x, ll.y, ur.x, ur.y);
        })
    });
    c.emit();
    Ok(())
}

async fn celtscale(c: Ctx, _a: Args) -> CmdResult<()> {
    let cur = c.doc(|d| d.settings.celtscale);
    let v = c
        .number(
            ps("Enter new value for CELTSCALE")
                .default_num(cur)
                .min(1e-9),
        )
        .await?;
    if let Some(x) = v.value().filter(|x| *x != 0.0) {
        c.setvar("CELTSCALE", x)?;
    }
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(
        def("Drafting", "LIMITS", run(limits))
            .title("Drawing Limits")
            .icon("grid")
            .desc("Sets and controls the grid display limits in the current model space."),
    );
    r.define(
        def("Properties", "CELTSCALE", run(celtscale))
            .title("Object Linetype Scale")
            .icon("ltscale")
            .desc("Sets the linetype scale for new objects drawn from now on."),
    );
}
