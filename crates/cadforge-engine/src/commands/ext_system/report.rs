//! Drawing reports: STATUS, TIME, DBLIST.
use super::{plural, ps, Shared};
use crate::app::Event;
use crate::commands::helpers::num;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::parse::{os_full, OS_BITS};
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::{Entity, Kind};
use cadforge_core::geom::*;
use cadforge_core::style::{describe_color, fmt_num, lw_display};

// ---------------------------------------------------------------- STATUS

fn xy(prec: usize, x: f64, y: f64) -> String {
    format!(
        "X: {:>10}  Y: {:>10}",
        fmt_num(x, prec, false),
        fmt_num(y, prec, false)
    )
}

async fn status(c: Ctx, _a: Args) -> CmdResult<()> {
    let lines: Vec<String> = c.app(|a| {
        let ed = a.editor().expect("a command runs on an open drawing");
        let (d, sv) = (&ed.doc, &a.sysvars);
        let prec = d.settings.units.precision.max(0) as usize;
        let objects = d.model.entities.len()
            + d.layouts.iter().map(|l| l.entities.len()).sum::<usize>()
            + d.solids.len();
        let l = d.settings.limits;
        let on_off = |b: bool| if b { "on" } else { "off" };
        let mut out = vec![format!("{} in {}", plural(objects, "object"), d.name)];
        out.push(format!(
            "Model space limits are  {}  ({})",
            xy(prec, l.min_x, l.min_y),
            if l.on { "On" } else { "Off" }
        ));
        out.push(format!(
            "                        {}",
            xy(prec, l.max_x, l.max_y)
        ));
        match ed.extents() {
            Some(b) => {
                out.push(format!(
                    "Model space uses        {}",
                    xy(prec, b.min_x, b.min_y)
                ));
                out.push(format!(
                    "                        {}",
                    xy(prec, b.max_x, b.max_y)
                ));
            }
            None => out.push("Model space uses        **Nothing**".into()),
        }
        if ed.view.is_plan() {
            let b = ed.view.world_bounds();
            out.push(format!(
                "Display shows           {}",
                xy(prec, b.min_x, b.min_y)
            ));
            out.push(format!(
                "                        {}",
                xy(prec, b.max_x, b.max_y)
            ));
        } else {
            out.push(format!(
                "Display shows           {} view",
                ed.view.preset_name()
            ));
        }
        out.push(format!(
            "Snap resolution is      {}",
            xy(prec, sv.real("SNAPUNIT_X"), sv.real("SNAPUNIT_Y"))
        ));
        out.push(format!(
            "Grid spacing is         {}",
            xy(prec, sv.real("GRIDUNIT_X"), sv.real("GRIDUNIT_Y"))
        ));
        out.push(format!(
            "Current space:          {}",
            if d.is_model() {
                "Model space".to_string()
            } else {
                format!("Paper space ({})", d.space)
            }
        ));
        out.push(format!("Current layer:          \"{}\"", d.cur.layer));
        out.push(format!(
            "Current color:          {}",
            describe_color(d.cur.color)
        ));
        out.push(format!(
            "Current linetype:       {}",
            d.cur
                .ltype
                .as_deref()
                .map_or("BYLAYER".to_string(), |t| format!("\"{t}\""))
        ));
        out.push(format!("Current lineweight:     {}", lw_display(d.cur.lw)));
        out.push(format!(
            "Linetype scale:         {}  (new objects {})",
            num(d.settings.ltscale, 4),
            num(d.settings.celtscale, 4)
        ));
        out.push(format!(
            "Units:                  {}, precision {}, angle precision {}, insertion units {}",
            d.settings.units.kind,
            d.settings.units.precision,
            d.settings.units.ang_precision,
            d.settings.units.ins_units
        ));
        out.push(format!(
            "Fill {}  Grid {}  Ortho {}  Snap {}  Polar {}",
            on_off(sv.int("FILLMODE") != 0),
            on_off(sv.grid_on()),
            on_off(sv.ortho()),
            on_off(sv.snap_on()),
            on_off(sv.polar())
        ));
        let modes: Vec<&str> = OS_BITS
            .iter()
            .filter(|(code, bit)| *bit & (sv.int("OSMODE") as u32) != 0 && *code != "QUI")
            .map(|(code, _)| os_full(code))
            .collect();
        out.push(format!(
            "Object snap modes:      {}{}",
            if modes.is_empty() {
                "None".to_string()
            } else {
                modes.join(", ")
            },
            if sv.osnap_on() {
                ""
            } else {
                "  (running object snap is off)"
            }
        ));
        out.push("Free memory:            not available".into());
        out
    });
    c.log(&lines.join("\n"));
    c.app(|a| a.emit(Event::ShowHistory));
    Ok(())
}

// ---------------------------------------------------------------- TIME

/// `0 days 00:12:34.500` from milliseconds.
fn clock(ms: f64) -> String {
    let t = ms.max(0.0).round() as u64;
    format!(
        "{} days {:02}:{:02}:{:02}.{:03}",
        t / 86_400_000,
        t / 3_600_000 % 24,
        t / 60_000 % 60,
        t / 1000 % 60,
        t % 1000
    )
}

fn time_report(st: &Shared, id: u64, now: f64) -> Vec<String> {
    let t = st.borrow();
    let Some(d) = t.docs.get(&id) else {
        return vec![];
    };
    let elapsed = d.timer_ms + if d.timer_on { now - d.timer_since } else { 0.0 };
    vec![
        format!("Current session time:    {}", clock(now)),
        "Times for this drawing (session clock, tracked at command boundaries):".into(),
        format!("  Opened:                {}", clock(d.opened)),
        format!(
            "  Last updated:          {}",
            d.updated
                .map_or("<no modifications yet>".to_string(), clock)
        ),
        format!("  Time since opened:     {}", clock(now - d.opened)),
        format!(
            "  Elapsed timer ({}):    {}",
            if d.timer_on { "on" } else { "off" },
            clock(elapsed)
        ),
    ]
}

async fn time(st: Shared, c: Ctx, _a: Args) -> CmdResult<()> {
    let id = c.ed(|e| e.id);
    let mut show = true;
    loop {
        let now = c.app(|a| a.now);
        if show {
            c.log(&time_report(&st, id, now).join("\n"));
        }
        show = false;
        let k = c
            .keyword(ps("Enter option").options(&["Display", "ON", "OFF", "Reset"]))
            .await?;
        let now = c.app(|a| a.now);
        let mut t = st.borrow_mut();
        let Some(d) = t.docs.get_mut(&id) else {
            return Ok(());
        };
        match k.kw() {
            Some("Display") => show = true,
            Some("ON") if !d.timer_on => {
                d.timer_on = true;
                d.timer_since = now;
                c.log("Timer is on.");
            }
            Some("OFF") if d.timer_on => {
                d.timer_ms += now - d.timer_since;
                d.timer_on = false;
                c.log("Timer is off.");
            }
            Some("Reset") => {
                (d.timer_ms, d.timer_since) = (0.0, now);
                c.log("Timer reset.");
            }
            Some(_) => {}
            None => return Ok(()),
        }
    }
}

// ---------------------------------------------------------------- DBLIST

fn describe(d: &Drawing, e: &Entity) -> Vec<String> {
    let f = |v: f64| num(v, 4);
    let p = |q: Pt| format!("X={}  Y={}", f(q.x), f(q.y));
    let mut out = vec![
        format!("{:<14} Layer: \"{}\"", e.label().to_uppercase(), e.layer),
        format!("               Handle = {:X}", e.id),
    ];
    match &e.kind {
        Kind::Line { a, b } => out.push(format!(
            "               from {}  to {}  length {}",
            p(*a),
            p(*b),
            f(dist(*a, *b))
        )),
        Kind::Circle { c, r } => {
            out.push(format!("               center {}  radius {}", p(*c), f(*r)))
        }
        Kind::Arc { c, r, a0, a1 } => out.push(format!(
            "               center {}  radius {}  from {} to {} degrees",
            p(*c),
            f(*r),
            f(deg(*a0)),
            f(deg(*a1))
        )),
        Kind::Pline { v, closed } => out.push(format!(
            "               {} polyline, {} vertices, length {}",
            if *closed { "closed" } else { "open" },
            v.len(),
            f(pline_length(v, *closed))
        )),
        Kind::Text(t) => out.push(format!(
            "               at {}  height {}  \"{}\"",
            p(t.p),
            f(t.h),
            t.s
        )),
        Kind::Insert(i) => out.push(format!(
            "               block \"{}\" at {}  scale {},{}  rotation {}",
            i.name,
            p(i.p),
            f(i.sx),
            f(i.sy),
            f(deg(i.rot))
        )),
        Kind::Point { p: q } => out.push(format!("               at {}", p(*q))),
        _ => {}
    }
    if d.layer(&e.layer).is_none() {
        out.push("               (layer is not defined)".into());
    }
    out
}

async fn dblist(c: Ctx, _a: Args) -> CmdResult<()> {
    let lines: Vec<String> = c.doc(|d| {
        let ents = d
            .model
            .entities
            .iter()
            .chain(d.layouts.iter().flat_map(|l| l.entities.iter()));
        let solids = d.solids.iter().map(|s| {
            vec![
                format!("{:<14} Layer: \"{}\"", "3D SOLID", s.layer),
                format!("               Handle = {:X}", s.id),
            ]
        });
        ents.map(|e| describe(d, e))
            .chain(solids)
            .flatten()
            .collect()
    });
    if lines.is_empty() {
        c.log("The drawing database is empty.");
    } else {
        c.log(&lines.join("\n"));
    }
    c.app(|a| a.emit(Event::ShowHistory));
    Ok(())
}

pub(super) fn register(r: &mut Registry, st: &Shared) {
    r.define(
        def("Inquiry", "STATUS", run(status))
            .title("Status")
            .icon("list")
            .desc("Displays drawing statistics, modes, and extents."),
    );
    let s = st.clone();
    r.define(
        def("Inquiry", "TIME", run(move |c, a| time(s.clone(), c, a)))
            .title("Time")
            .icon("list")
            .desc("Displays the date and time statistics of a drawing."),
    );
    r.define(
        def("Inquiry", "DBLIST", run(dblist))
            .title("Database List")
            .icon("list")
            .desc("Lists database information for every object in the drawing."),
    );
}
