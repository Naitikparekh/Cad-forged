//! View commands: HIDE, SHADEMODE, VISUALSTYLES, CLEANSCREENOFF, TEXTSCR, GRAPHSCR, VIEWCUBE, NAVVCUBE, NAVSWHEEL,
//! PLAN, VPOINT, DVIEW.
use super::{ps, ui_action};
use crate::commands::{def, run};
use crate::ctx::*;
use crate::parse::parse_number;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::geom::*;
use cadforge_core::solid::V3;

/// The visual styles the renderer knows, with their VISUALSTYLES option names.
const STYLES: [(&str, &str); 5] = [("2dwireframe", "2D Wireframe"), ("Wireframe", "Wireframe"), ("Hidden", "Hidden"), ("Shaded", "Shaded"), ("shaded with Edges", "Shaded with Edges")];

fn set_style(c: &Ctx, style: &str) {
    c.ed(|e| e.visual_style = style.to_string());
    c.emit();
}

fn style_label(style: &str) -> &'static str {
    STYLES.iter().find(|(_, s)| *s == style).map_or("2dwireframe", |(l, _)| *l)
}

async fn hide(c: Ctx, _a: Args) -> CmdResult<()> {
    set_style(&c, "Hidden");
    Ok(())
}

/// SHADEMODE options and the visual style each one selects.
const SHADE: [(&str, &str); 5] = [("2D wireframe", "2D Wireframe"), ("3D wireframe", "Wireframe"), ("Hidden", "Hidden"), ("Realistic", "Shaded"), ("Conceptual", "Shaded with Edges")];

async fn shademode(c: Ctx, _a: Args) -> CmdResult<()> {
    let cur = c.ed(|e| e.visual_style.clone());
    let default = SHADE.iter().find(|(_, s)| *s == cur).map_or("2D wireframe", |(l, _)| *l);
    let labels: Vec<&str> = SHADE.iter().map(|(l, _)| *l).collect();
    let k = c.keyword(ps("Enter option").options(&labels).default_str(default)).await?;
    if let Some((label, style)) = SHADE.iter().find(|(l, _)| Some(*l) == k.kw()) {
        set_style(&c, style);
        if !label.ends_with("wireframe") && *label != "Hidden" {
            c.log(&format!("{label} is shown with the {style} visual style."));
        }
    }
    Ok(())
}

async fn visualstyles(c: Ctx, _a: Args) -> CmdResult<()> {
    let cur = c.ed(|e| e.visual_style.clone());
    let mut labels = vec!["?"];
    labels.extend(STYLES.iter().map(|(l, _)| *l));
    let k = c.keyword(ps("Enter a visual style option").options(&labels).default_str(style_label(&cur))).await?;
    match k.kw() {
        Some("?") => {
            let lines: Vec<String> = STYLES.iter().map(|(_, s)| format!("  {s}{}", if *s == cur { "  (current)" } else { "" })).collect();
            c.log(&lines.join("\n"));
        }
        Some(l) => {
            if let Some((_, s)) = STYLES.iter().find(|(x, _)| *x == l) {
                set_style(&c, s);
            }
        }
        None => {}
    }
    Ok(())
}

// ---------------------------------------------------------------- interface switches

async fn cleanscreenoff(c: Ctx, _a: Args) -> CmdResult<()> {
    ui_action(&c, "CLEANSCREENOFF", "CLEANSCREENOFF", Args::none());
    Ok(())
}

async fn textscr(c: Ctx, _a: Args) -> CmdResult<()> {
    ui_action(&c, "TEXTSCR", "TEXTSCR", Args::none());
    Ok(())
}

async fn graphscr(c: Ctx, _a: Args) -> CmdResult<()> {
    ui_action(&c, "GRAPHSCR", "GRAPHSCR", Args::none());
    Ok(())
}

fn report_flag(c: &Ctx, label: &str, var: &str) {
    c.log(&format!("<{label} {}>", if c.pref(var) != 0.0 { "on" } else { "off" }));
    c.emit();
}

async fn viewcube(c: Ctx, _a: Args) -> CmdResult<()> {
    c.setvar("NAVVCUBE", if c.pref("NAVVCUBE") != 0.0 { 0.0 } else { 1.0 })?;
    report_flag(&c, "ViewCube", "NAVVCUBE");
    Ok(())
}

async fn navvcube(c: Ctx, _a: Args) -> CmdResult<()> {
    let default = if c.pref("NAVVCUBE") != 0.0 { "ON" } else { "OFF" };
    let k = c.keyword(ps("Enter an option").options(&["ON", "OFF", "Settings"]).default_str(default)).await?;
    match k.kw() {
        Some("Settings") => ui_action(&c, "NAVVCUBE", "OPTIONS", Args::none()),
        Some(v) => {
            c.setvar("NAVVCUBE", if v == "ON" { 1.0 } else { 0.0 })?;
            report_flag(&c, "ViewCube", "NAVVCUBE");
        }
        None => {}
    }
    Ok(())
}

async fn navswheel(c: Ctx, _a: Args) -> CmdResult<()> {
    c.log_error("SteeringWheels are not available in this version.");
    c.setvar("NAVBAR", 1.0)?;
    c.log("The navigation bar (pan, zoom, orbit) is shown instead.");
    c.emit();
    Ok(())
}

// ---------------------------------------------------------------- viewpoints

/// Yaw and pitch of a camera looking at the drawing from direction `v` (the vector from the target to the viewer).
fn view_angles(v: V3) -> (f64, f64) {
    let h = v.x.hypot(v.y);
    (if h < 1e-9 { 0.0 } else { (-v.x).atan2(-v.y) }, v.z.atan2(h))
}

/// Direction from the target to the viewer for a yaw / pitch.
fn view_dir(yaw: f64, pitch: f64) -> V3 {
    V3::new(-yaw.sin() * pitch.cos(), -yaw.cos() * pitch.cos(), pitch.sin())
}

/// "x,y[,z]" typed as a vector or point.
fn parse_xyz(t: &str) -> Option<V3> {
    let v: Vec<f64> = t.split(',').map(parse_number).collect::<Option<_>>()?;
    match v.as_slice() {
        [x, y] => Some(V3::new(*x, *y, 0.0)),
        [x, y, z] => Some(V3::new(*x, *y, *z)),
        _ => None,
    }
}

fn aim(c: &Ctx, yaw: f64, pitch: f64) {
    c.view(|v| {
        v.push();
        v.yaw = yaw;
        v.pitch = pitch;
    });
    c.emit();
}

/// Angle from the XY plane and angle in the XY plane (degrees) of the direction toward the viewer.
fn camera_angles(c: &Ctx) -> (f64, f64) {
    let (yaw, pitch) = c.view(|v| (v.yaw, v.pitch));
    let d = view_dir(yaw, pitch);
    (deg(pitch), deg(d.y.atan2(d.x)))
}

async fn vpoint(c: Ctx, _a: Args) -> CmdResult<()> {
    // a text prompt: "Rotate" is matched by hand, because option clicks only answer keyword prompts
    let r = c.string(ps("Specify a view point or [Rotate]").default_str("display compass and tripod")).await?;
    let t = r.text.trim().to_lowercase();
    if r.entered {
        c.log_error("The compass and tripod are not available; enter a view point vector or use the Rotate option.");
    } else if !t.is_empty() && "rotate".starts_with(&t) {
        let (pitch0, xy0) = camera_angles(&c);
        let Some(xy) = c.number(ps("Enter angle in XY plane from X axis").default_num(xy0)).await?.value() else { return Ok(()) };
        let Some(z) = c.number(ps("Enter angle from XY plane").default_num(pitch0).min(-90.0).max(90.0)).await?.value() else { return Ok(()) };
        let (yaw, pitch) = view_angles(view_dir_xy(xy, z));
        aim(&c, yaw, pitch);
    } else {
        match parse_xyz(&t).filter(|v| v.x != 0.0 || v.y != 0.0 || v.z != 0.0) {
            Some(v) => {
                let (yaw, pitch) = view_angles(v);
                aim(&c, yaw, pitch);
            }
            None => c.log_error("Invalid view point."),
        }
    }
    Ok(())
}

/// Direction toward the viewer from an angle in the XY plane and an angle from it (both degrees).
fn view_dir_xy(xy: f64, z: f64) -> V3 {
    let (a, p) = (rad(xy), rad(z));
    V3::new(p.cos() * a.cos(), p.cos() * a.sin(), p.sin())
}

async fn plan(c: Ctx, _a: Args) -> CmdResult<()> {
    let k = c.keyword(ps("Enter an option").options(&["Current ucs", "Ucs", "World"]).default_str("Current ucs")).await?;
    if k.kw() == Some("Ucs") {
        c.log_error("Named UCSs are not available in this version; use Current or World.");
        return Ok(());
    }
    c.ed(|e| {
        e.view.push();
        e.view.set_preset("top");
        let mut b = e.extents();
        for s in &e.doc.solids {
            let pts: Vec<Pt> = s.vertices.iter().map(|v| pt(v.x, v.y)).collect();
            if !pts.is_empty() {
                b = Bounds::union_opt(b, Some(Bounds::of_points(&pts)));
            }
        }
        e.view.fit(b, 0.06);
        e.zoomed = true;
    });
    c.emit();
    Ok(())
}

// ---------------------------------------------------------------- DVIEW

const DVIEW_OPTS: [&str; 12] = ["CAmera", "TArget", "Distance", "POints", "PAn", "Zoom", "TWist", "CLip", "Hide", "Off", "Undo", "eXit"];

async fn dview(c: Ctx, _a: Args) -> CmdResult<()> {
    // objects are not previewed separately: the whole drawing is shown while the view changes
    let mut changes = 0;
    loop {
        let k = c.keyword(ps("Enter option").options(&DVIEW_OPTS).default_str("eXit")).await?;
        match k.kw() {
            Some(o @ ("CAmera" | "TArget")) => {
                // the target direction is the camera direction reversed
                let sign = if o == "CAmera" { 1.0 } else { -1.0 };
                let (pitch0, xy0) = camera_angles(&c);
                let Some(z) = c.number(ps("Enter angle from XY plane").default_num(sign * pitch0).min(-90.0).max(90.0)).await?.value() else { continue };
                let Some(xy) = c.number(ps("Enter angle in XY plane from X axis").default_num(if sign > 0.0 { xy0 } else { xy0 + 180.0 })).await?.value() else { continue };
                let (yaw, pitch) = view_angles(view_dir_xy(if sign > 0.0 { xy } else { xy + 180.0 }, sign * z));
                aim(&c, yaw, pitch);
                changes += 1;
            }
            Some("POints") => {
                let cur = c.view(|v| v.target);
                let t = c.string(ps("Specify target point").default_str(&format!("{},{},{}", cur.x, cur.y, cur.z))).await?.text;
                let cam = c.string(ps("Specify camera point")).await?.text;
                match (parse_xyz(&t), parse_xyz(&cam)) {
                    (Some(t), Some(p)) if (p.x - t.x).hypot(p.y - t.y).hypot(p.z - t.z) > 1e-9 => {
                        let (yaw, pitch) = view_angles(V3::new(p.x - t.x, p.y - t.y, p.z - t.z));
                        c.view(|v| {
                            v.push();
                            v.target = t;
                            v.yaw = yaw;
                            v.pitch = pitch;
                        });
                        c.emit();
                        changes += 1;
                    }
                    _ => c.log_error("Invalid camera or target point."),
                }
            }
            Some("PAn") => {
                c.sub_forced("-PAN", Args::none()).await;
                changes += 1;
            }
            Some("Zoom") => {
                let Some(f) = c.number(ps("Specify zoom scale factor").default_num(1.0).min(1e-9)).await?.value() else { continue };
                c.view(|v| {
                    v.push();
                    v.scale *= f;
                });
                c.emit();
                changes += 1;
            }
            Some("Hide") => set_style(&c, "Hidden"),
            Some("Undo") if changes > 0 => {
                c.view(|v| v.pop_previous());
                changes -= 1;
                c.emit();
            }
            Some("Undo") => c.log_error("Nothing to undo."),
            Some("Distance") => c.log_error("Perspective views are not supported; the view is always a parallel projection."),
            Some("Off") => c.log("Perspective is off: the view is always a parallel projection."),
            Some("TWist") => c.log_error("View twist is not supported."),
            Some("CLip") => c.log_error("Clipping planes are not supported."),
            _ => return Ok(()),
        }
    }
}

pub(super) fn register(r: &mut Registry) {
    r.define(def("View", "HIDE", run(hide)).aliases(&["HI"]).title("Hide").icon("view").desc("Regenerates a three-dimensional model with hidden lines suppressed.").once());
    r.define(def("View", "SHADEMODE", run(shademode)).aliases(&["SHA"]).title("Shade Mode").icon("view").desc("Starts the VSCURRENT command: sets the visual style in the current viewport."));
    r.define(def("View", "VISUALSTYLES", run(visualstyles)).aliases(&["VSM", "-VISUALSTYLES"]).title("Visual Styles").icon("view").desc("Creates and modifies visual styles and applies a visual style to a viewport (command-line form)."));
    r.define(def("View", "CLEANSCREENOFF", run(cleanscreenoff)).title("Clean Screen Off").icon("cleanscreen").desc("Restores the display state that was in effect before CLEANSCREENON.").transparent().once());
    r.define(def("Utilities", "TEXTSCR", run(textscr)).title("Text Window").desc("Displays the text window.").transparent().once());
    r.define(def("Utilities", "GRAPHSCR", run(graphscr)).title("Graphics Screen").desc("Switches from the text window to the drawing area.").transparent().once());
    r.define(def("View", "VIEWCUBE", run(viewcube)).title("ViewCube").icon("viewcube").desc("Turns the ViewCube on or off.").transparent());
    r.define(def("View", "NAVVCUBE", run(navvcube)).title("ViewCube Display").icon("viewcube").desc("Controls the visibility and display properties of the ViewCube.").transparent());
    r.define(def("View", "NAVSWHEEL", run(navswheel)).title("SteeringWheels").icon("orbit").desc("Displays a wheel that contains a collection of view tools.").transparent().once());
    r.define(def("View", "PLAN", run(plan)).title("Plan View").icon("view").desc("Displays the plan view of a specified user coordinate system."));
    r.define(def("View", "VPOINT", run(vpoint)).aliases(&["-VP"]).title("Viewpoint").icon("orbit").desc("Sets the viewing direction for a 3D visualization of the drawing."));
    r.define(def("View", "DVIEW", run(dview)).aliases(&["DV"]).title("Dynamic View").icon("orbit").desc("Defines parallel projection or perspective views by using a camera and target."));
}
