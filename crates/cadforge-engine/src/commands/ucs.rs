//! User coordinate systems (2D): UCS and UCSICON.
use super::{def, run};
use crate::app::Event;
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn set_ucs(c: &Ctx, u: Ucs) {
    c.doc_mut(|d| {
        if d.ucs != u {
            let prev = d.ucs;
            d.ucs_prev.push(prev);
            d.ucs = u;
            d.touch();
        }
    });
    c.app(|a| a.emit(Event::Regen));
    c.emit();
}

fn describe(u: &Ucs) -> String {
    if u.is_world() {
        "World".into()
    } else {
        format!("origin {:.4},{:.4}  X axis at {:.4}°", u.origin.x, u.origin.y, deg(norm_angle(u.angle)))
    }
}

/// Align the UCS to an object: a line's start and direction, an arc/circle/ellipse centre, a text or block origin.
fn from_object(e: &Entity, pick: Pt, doc: &cadforge_core::drawing::Drawing) -> Option<Ucs> {
    match &e.kind {
        Kind::Line { a, b } => Some(Ucs { origin: *a, angle: angle_of(*a, *b) }),
        Kind::Circle { c, .. } => Some(Ucs { origin: *c, angle: doc.ucs.angle }),
        Kind::Arc { c, r, a0, .. } => {
            // the end of the arc nearest the pick becomes the origin side: X axis through the start point
            let _ = r;
            Some(Ucs { origin: *c, angle: *a0 })
        }
        Kind::Ellipse(el) => Some(Ucs { origin: el.c, angle: el.m.y.atan2(el.m.x) }),
        Kind::Text(t) => Some(Ucs { origin: t.p, angle: t.rot }),
        Kind::Insert(i) => Some(Ucs { origin: i.p, angle: i.rot }),
        Kind::Point { p } => Some(Ucs { origin: *p, angle: doc.ucs.angle }),
        Kind::Pline { .. } => {
            let segs = segs_of(e, doc);
            let s = segs.iter().min_by(|a, b| a.nearest(pick).2.total_cmp(&b.nearest(pick).2))?;
            match *s {
                Seg::L { a, b, .. } => Some(Ucs { origin: a, angle: angle_of(a, b) }),
                Seg::A { c, .. } => Some(Ucs { origin: c, angle: doc.ucs.angle }),
            }
        }
        _ => None,
    }
}

async fn ucs(c: Ctx, args: Args) -> CmdResult<()> {
    let opts = ["New", "Move", "orthoGraphic", "Prev", "Restore", "Save", "Del", "Apply", "?", "World"];
    let cur = c.doc(|d| d.ucs);
    let kw = match args.str("option") {
        Some(o) => o.to_string(),
        None => {
            let r = c.keyword(ps("Enter an option [New/Move/orthoGraphic/Prev/Restore/Save/Del/Apply/?/World]").options(&opts).default_str("World").allow_enter()).await?;
            r.kw().unwrap_or("World").to_string()
        }
    };
    match kw.as_str() {
        "World" => set_ucs(&c, Ucs::default()),
        "Move" => {
            let r = c.point(ps("Specify new origin point or [Zdepth]").options(&["Zdepth"]).no_ortho()).await?;
            if r.kw() == Some("Zdepth") {
                c.log_error("Z depth has no effect: the UCS lies in the XY plane of this 2D drawing.");
            } else if let Some(p) = r.pt() {
                set_ucs(&c, Ucs { origin: p, angle: cur.angle });
            }
        }
        "New" => {
            let r = c.point(ps("Specify origin of new UCS or [ZAxis/3point/OBject/Face/View/X/Y/Z]").options(&["ZAxis", "3point", "OBject", "Face", "View", "X", "Y", "Z"]).allow_enter().no_ortho()).await?;
            match r.kw() {
                Some("3point") => {
                    let Some(o) = c.point(ps("Specify new origin point").default_pt(cur.origin)).await?.pt() else { return Ok(()) };
                    let Some(x) = c.point(ps("Specify point on positive portion of X-axis").base(o)).await?.pt() else { return Ok(()) };
                    let Some(_y) = c.point(ps("Specify point on positive-Y portion of the UCS XY plane").base(o)).await?.pt() else { return Ok(()) };
                    if dist(o, x) < 1e-9 {
                        c.log_error("The X axis point must differ from the origin.");
                    } else {
                        set_ucs(&c, Ucs { origin: o, angle: angle_of(o, x) });
                    }
                }
                Some("OBject") => {
                    let r = c.entity(ps("Select object to align UCS")).await?;
                    if let EntR::Entity { id, pt: pick, .. } = r {
                        match c.doc(|d| d.by_id(id).and_then(|e| from_object(e, pick, d))) {
                            Some(u) => set_ucs(&c, u),
                            None => c.log_error("That object type cannot define a UCS."),
                        }
                    }
                }
                Some("View") => set_ucs(&c, Ucs { origin: cur.origin, angle: 0.0 }),
                Some("Z") => {
                    let a = c.number(ps("Specify rotation angle about Z axis").default_num(90.0)).await?;
                    if let Some(a) = a.value() {
                        set_ucs(&c, Ucs { origin: cur.origin, angle: cur.angle + rad(a) });
                    }
                }
                Some("X") | Some("Y") | Some("ZAxis") | Some("Face") => c.log_error("This option tilts the UCS out of the XY plane; only rotation about Z, origin changes and object/3-point alignment are available in 2D drawings."),
                _ => {
                    if let Some(p) = r.pt() {
                        set_ucs(&c, Ucs { origin: p, angle: cur.angle });
                    }
                }
            }
        }
        "orthoGraphic" => {
            let r = c.keyword(ps("Enter an option [Top/Bottom/Front/BAck/Left/Right]").options(&["Top", "Bottom", "Front", "BAck", "Left", "Right"]).default_str("Top").allow_enter()).await?;
            if r.kw().unwrap_or("Top") == "Top" {
                set_ucs(&c, Ucs { origin: cur.origin, angle: 0.0 });
            } else {
                c.log_error("Only the Top orthographic UCS lies in the XY plane of a 2D drawing.");
            }
        }
        "Prev" => {
            let prev = c.doc_mut(|d| d.ucs_prev.pop());
            match prev {
                Some(u) => {
                    c.doc_mut(|d| {
                        d.ucs = u;
                        d.touch();
                    });
                    c.app(|a| a.emit(Event::Regen));
                    c.emit();
                }
                None => c.log_error("There is no previous UCS."),
            }
        }
        "Save" => {
            let n = match args.str("name") {
                Some(n) => n.to_string(),
                None => c.string(ps("Enter name to save current UCS or [?]")).await?.text,
            };
            if n.trim().is_empty() || n == "?" {
                list(&c);
            } else {
                c.doc_mut(|d| {
                    d.ucs_named.insert(n.trim().to_string(), d.ucs);
                    d.touch();
                });
                c.log(&format!("UCS \"{}\" saved.", n.trim()));
            }
        }
        "Restore" => {
            let n = match args.str("name") {
                Some(n) => n.to_string(),
                None => c.string(ps("Enter name of UCS to restore or [?]")).await?.text,
            };
            if n.trim().is_empty() || n == "?" {
                list(&c);
            } else {
                match c.doc(|d| d.ucs_named.iter().find(|(k, _)| k.eq_ignore_ascii_case(n.trim())).map(|(_, u)| *u)) {
                    Some(u) => set_ucs(&c, u),
                    None => c.log_error(&format!("There is no saved UCS named \"{}\".", n.trim())),
                }
            }
        }
        "Del" => {
            let n = match args.str("name") {
                Some(n) => n.to_string(),
                None => c.string(ps("Enter UCS name(s) to delete")).await?.text,
            };
            let removed = c.doc_mut(|d| {
                let key = d.ucs_named.keys().find(|k| k.eq_ignore_ascii_case(n.trim())).cloned();
                key.map(|k| d.ucs_named.shift_remove(&k).is_some()).unwrap_or(false)
            });
            if removed {
                c.log(&format!("UCS \"{}\" deleted.", n.trim()));
            } else {
                c.log_error(&format!("There is no saved UCS named \"{}\".", n.trim()));
            }
        }
        "Apply" => c.log("The UCS applies to the whole drawing (there are no viewport-specific UCS settings)."),
        "?" => list(&c),
        _ => {}
    }
    Ok(())
}

fn list(c: &Ctx) {
    let (cur, named) = c.doc(|d| (d.ucs, d.ucs_named.iter().map(|(k, u)| format!("{k}: {}", describe(u))).collect::<Vec<_>>()));
    let mut lines = vec![format!("Current UCS: {}", describe(&cur))];
    lines.extend(named);
    c.log(&lines.join("\n"));
}

async fn ucsicon(c: Ctx, args: Args) -> CmdResult<()> {
    let kw = match args.str("option") {
        Some(o) => o.to_string(),
        None => c.keyword(ps("Enter an option [ON/OFF/All/Noorigin/ORigin/Properties]").options(&["ON", "OFF", "All", "Noorigin", "ORigin", "Properties"]).default_str("ON").allow_enter()).await?.kw().unwrap_or("ON").to_string(),
    };
    match kw.as_str() {
        "OFF" => {
            let _ = c.setvar("UCSICON", 0.0);
        }
        "ON" | "All" => {
            let _ = c.setvar("UCSICON", 1.0);
        }
        "Noorigin" => {
            let _ = c.setvar("UCSICON", 1.0);
            let _ = c.setvar("UCSORG", 0.0);
        }
        "ORigin" => {
            let _ = c.setvar("UCSICON", 1.0);
            let _ = c.setvar("UCSORG", 1.0);
        }
        _ => c.log("The icon style is fixed: 2D wireframe axes with the X and Y labels."),
    }
    c.app(|a| a.emit(Event::Regen));
    c.emit();
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(def("View", "UCS", run(ucs)).title("UCS").desc("Manages user coordinate systems.").icon("ucs"));
    r.define(def("View", "UCSICON", run(ucsicon)).title("UCS Icon").desc("Controls the visibility and placement of the UCS icon.").icon("ucs").transparent());
}
