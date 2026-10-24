//! Isometric drafting: ISODRAFT switches the isometric snap and grid on or off, ISOPLANE picks the plane ortho follows.
use super::ps;
use crate::app::Event;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;

const PLANES: [&str; 3] = ["Left", "Top", "Right"];

fn plane_name(i: f64) -> &'static str {
    PLANES[(i as i64).clamp(0, 2) as usize]
}

fn set_plane(c: &Ctx, i: usize) -> CmdResult<()> {
    c.setvar("ISOPLANE", i as f64)?;
    c.log(&format!("Current isoplane: {}", PLANES[i]));
    c.app(|a| a.emit(Event::Regen));
    c.emit();
    Ok(())
}

/// ISOPLANE: Left, Top or Right; Enter (or Toggle) moves on to the next plane, like F5.
async fn isoplane(c: Ctx, a: Args) -> CmdResult<()> {
    let cur = c.pref("ISOPLANE");
    let next = PLANES[((cur as i64 + 1).rem_euclid(3)) as usize];
    let kw = match a.str("option") {
        Some(o) => o.to_string(),
        None => c.keyword(ps("Enter isometric plane setting [Left/Top/Right/Toggle]").options(&["Left", "Top", "Right", "Toggle"]).default_str(next).allow_enter()).await?.kw().unwrap_or(next).to_string(),
    };
    let i = match kw.to_ascii_uppercase().as_str() {
        "L" | "LEFT" => 0,
        "T" | "TOP" => 1,
        "R" | "RIGHT" => 2,
        _ => ((cur as i64 + 1).rem_euclid(3)) as usize,
    };
    set_plane(&c, i)
}

/// ISODRAFT: Orthographic turns isometric drafting off; isoLeft / isoTop / isoRight turn it on in that plane.
async fn isodraft(c: Ctx, a: Args) -> CmdResult<()> {
    let iso = c.pref("SNAPSTYL") == 1.0;
    let cur = if iso { format!("iso{}", plane_name(c.pref("ISOPLANE"))) } else { "Orthographic".to_string() };
    let kw = match a.str("option") {
        Some(o) => o.to_string(),
        None => c.keyword(ps("Enter option [Orthographic/isoLeft/isoTop/isoRight]").options(&["Orthographic", "isoLeft", "isoTop", "isoRight"]).default_str(&cur).allow_enter()).await?.kw().unwrap_or(&cur).to_string(),
    };
    let plane = match kw.to_ascii_uppercase().trim_start_matches("ISO") {
        "LEFT" | "L" => Some(0),
        "TOP" | "T" => Some(1),
        "RIGHT" | "R" => Some(2),
        _ => None,
    };
    match plane {
        Some(i) => {
            c.setvar("SNAPSTYL", 1.0)?;
            c.log("Isometric drafting is on.");
            set_plane(&c, i)
        }
        None => {
            c.setvar("SNAPSTYL", 0.0)?;
            c.log("Isometric drafting is off: the snap and grid are rectangular.");
            c.app(|a| a.emit(Event::Regen));
            c.emit();
            Ok(())
        }
    }
}

pub fn register(r: &mut Registry) {
    r.define(def("Drafting", "ISOPLANE", run(isoplane)).title("Isometric Plane").icon("grid").desc("Specifies the current isometric plane (Left, Top or Right) that ortho and the cursor follow.").transparent());
    r.define(def("Drafting", "ISODRAFT", run(isodraft)).title("Isometric Drafting").icon("grid").desc("Turns isometric drafting on or off and sets the current isometric plane.").transparent());
}
