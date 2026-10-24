//! View commands: ZOOM, PAN, REGEN, VIEW, 3DORBIT (port of `src/commands/view.js`).
use super::{def, run};
use crate::app::Event;
use crate::ctx::*;
use crate::registry::{CommandDef, Registry};
use crate::runtime::*;
use crate::select::rect_of;
use cadforge_core::drawing::NamedView;
use cadforge_core::entities::bounds_of;
use cadforge_core::geom::*;
use cadforge_core::solid::V3;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn dv(name: &str, f: crate::registry::RunFn) -> CommandDef {
    def("View", name, f).transparent()
}

fn zoom_rect(c: &Ctx, rect: Bounds, margin: f64) {
    c.ed(|e| {
        e.view.push();
        e.view.fit(Some(rect), margin);
        e.zoomed = true;
    });
    c.emit();
}

fn apply_scale(c: &Ctx, n: f64, rel: Option<NumRel>) {
    c.ed(|e| {
        e.view.push();
        if let (Some(NumRel::Xp), Some(m)) = (rel, e.mspace.as_ref()) {
            // inside a layout viewport nXP sets the viewport scale: n sheet units per model unit
            let f = m.paper_view.scale * n / e.view.scale;
            let centre = e
                .mspace_centre()
                .unwrap_or(Pt::new(e.view.w / 2.0, e.view.h / 2.0));
            e.view.zoom_at(centre.x, centre.y, f);
            e.sync_mspace_view();
        } else if rel.is_some() {
            let (w, h) = (e.view.w, e.view.h);
            e.view.zoom_at(w / 2.0, h / 2.0, n);
        } else {
            let b = e.extents().unwrap_or(Bounds::new(0.0, 0.0, 420.0, 297.0));
            let mut base = e.view.clone();
            base.fit(Some(b), 0.06);
            e.view.scale = base.scale * n;
        }
    });
    c.emit();
}

fn factor(c: &Ctx, f: f64) {
    c.ed(|e| {
        e.view.push();
        let (w, h) = (e.view.w, e.view.h);
        e.view.zoom_at(w / 2.0, h / 2.0, f);
    });
    c.emit();
}

fn extents(c: &Ctx) {
    c.ed(|e| e.zoom_extents(0.06));
    c.emit();
}

fn previous(c: &Ctx) {
    if !c.view(|v| v.pop_previous()) {
        c.log_error("No previous view.");
    }
    c.emit();
}

async fn zoom(c: Ctx, args: Args) -> CmdResult<()> {
    let mode = args.str("mode").map(str::to_string);
    match mode.as_deref() {
        Some("extents") | Some("all") => return Ok(extents(&c)),
        Some("in") => return Ok(factor(&c, 2.0)),
        Some("out") => return Ok(factor(&c, 0.5)),
        Some("previous") => return Ok(previous(&c)),
        _ => {}
    }
    let r: PointR = match mode.as_deref() {
        None => {
            let r = c
                .point(
                    ps("Specify corner of window, enter a scale factor (nX or nXP), or")
                        .options(&[
                            "All", "Center", "Dynamic", "Extents", "Previous", "Scale", "Window",
                            "Object",
                        ])
                        .allow_enter()
                        .accept_number()
                        .no_ortho(),
                )
                .await?;
            if matches!(r, PointR::Enter(None)) {
                c.dialog(DialogReq::Realtime("zoom")).await?;
                return Ok(());
            }
            r
        }
        Some("window") => PointR::Kw("Window".into()),
        Some("object") => PointR::Kw("Object".into()),
        Some("scale") => PointR::Kw("Scale".into()),
        Some("center") => PointR::Kw("Center".into()),
        Some(_) => PointR::Enter(None),
    };
    if let PointR::Num { value, rel } = r {
        apply_scale(&c, value, rel);
        return Ok(());
    }
    if let PointR::Pt(p) = r {
        let Some(b) = c.corner(p, ps("Specify opposite corner")).await?.pt() else {
            return Ok(());
        };
        zoom_rect(&c, rect_of(p, b), 0.0);
        return Ok(());
    }
    match r.kw() {
        Some("All") | Some("Extents") => extents(&c),
        Some("Dynamic") | Some("Window") => {
            let Some(a) = c.point(ps("Specify first corner")).await?.pt() else {
                return Ok(());
            };
            let Some(b) = c.corner(a, ps("Specify opposite corner")).await?.pt() else {
                return Ok(());
            };
            zoom_rect(&c, rect_of(a, b), 0.0);
        }
        Some("Previous") => previous(&c),
        Some("Center") => {
            let Some(p) = c.point(ps("Specify center point")).await?.pt() else {
                return Ok(());
            };
            let (vh, vs) = c.view(|v| (v.h, v.scale));
            let h = c
                .number(
                    ps("Enter magnification or height")
                        .default_num(vh / vs)
                        .min(1e-12),
                )
                .await?;
            let Some(hv) = h.value() else { return Ok(()) };
            c.view(|v| {
                v.push();
                v.target = V3::new(p.x, p.y, v.target.z);
                v.scale = v.h / hv;
            });
            c.emit();
        }
        Some("Scale") => {
            let s = c
                .number(
                    ps("Enter a scale factor (nX or nXP)")
                        .default_num(1.0)
                        .min(1e-9),
                )
                .await?;
            if let Some(v) = s.value().filter(|v| *v != 0.0) {
                apply_scale(&c, v, Some(NumRel::X));
            }
        }
        Some("Object") => {
            let sel = c.select(ps("Select objects").pickfirst(true)).await?;
            if sel.ents.is_empty() {
                return Ok(());
            }
            let ents = c.entities(&sel.ents);
            let b = c.doc(|d| {
                let mut b: Option<Bounds> = None;
                for e in &ents {
                    b = Bounds::union_opt(b, bounds_of(e, d));
                }
                b
            });
            if let Some(b) = b {
                let pad = 1e-6f64.max((b.max_x - b.min_x + b.max_y - b.min_y) * 0.02);
                zoom_rect(&c, b.grow(pad), 0.04);
            }
        }
        _ => {}
    }
    Ok(())
}

async fn pan(c: Ctx, args: Args) -> CmdResult<()> {
    if let (Some(dx), Some(dy)) = (args.num("dx"), args.num("dy")) {
        c.view(|v| v.pan(dx, dy));
        c.emit();
        return Ok(());
    }
    c.dialog(DialogReq::Realtime("pan")).await?;
    Ok(())
}

async fn pan_disp(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(a) = c
        .point(ps("Specify base point or displacement"))
        .await?
        .pt()
    else {
        return Ok(());
    };
    let Some(b) = c
        .point(ps("Specify second point").base(a).allow_enter())
        .await?
        .pt()
    else {
        return Ok(());
    };
    let d = b - a;
    c.view(|v| v.target = V3::new(v.target.x - d.x, v.target.y - d.y, v.target.z));
    c.emit();
    Ok(())
}

async fn regen(c: Ctx, _a: Args) -> CmdResult<()> {
    c.app(|a| a.emit(Event::Regen));
    c.emit();
    Ok(())
}

async fn redraw(c: Ctx, _a: Args) -> CmdResult<()> {
    c.emit();
    Ok(())
}

async fn named_view(c: Ctx, _a: Args) -> CmdResult<()> {
    let r = c
        .keyword(
            ps("Enter an option [?/Delete/Orthographic/Restore/Save/sEttings/Window]")
                .options(&["?", "Delete", "Restore", "Save", "Window"])
                .allow_enter(),
        )
        .await?;
    let Some(kw) = r.kw().map(str::to_string) else {
        return Ok(());
    };
    if kw == "?" {
        let names: Vec<String> = c.doc(|d| d.views.keys().cloned().collect());
        c.log(&if names.is_empty() {
            "No named views.".to_string()
        } else {
            names.join("\n")
        });
        return Ok(());
    }
    if kw == "Window" {
        c.sub_forced(
            "ZOOM",
            Args::none().with("mode", ArgVal::Str("window".into())),
        )
        .await;
        return Ok(());
    }
    let n = c
        .string(ps(&format!("Enter view name to {}", kw.to_lowercase())))
        .await?
        .text;
    if n.is_empty() {
        return Ok(());
    }
    match kw.as_str() {
        "Save" => {
            let st = c.view(|v| v.state());
            c.doc_mut(|d| {
                d.change(|d| {
                    d.views.insert(n.clone(), NamedView::from_state(&st));
                })
            });
            c.log(&format!("View {n} saved."));
        }
        "Restore" => {
            let v = c.doc(|d| d.views.get(&n).copied());
            let Some(v) = v else {
                c.log_error(&format!("View {n} not found."));
                return Ok(());
            };
            c.view(|vw| {
                vw.push();
                vw.set_state(&v.to_state());
            });
            c.emit();
        }
        "Delete" => c.doc_mut(|d| {
            d.change(|d| {
                d.views.shift_remove(&n);
            })
        }),
        _ => {}
    }
    Ok(())
}

async fn orbit(c: Ctx, args: Args) -> CmdResult<()> {
    let pts: Vec<V3> = c.doc(|d| {
        d.solids
            .iter()
            .flat_map(|s| s.vertices.iter().copied())
            .collect()
    });
    if let Some(p) = args.str("preset") {
        c.view(|v| {
            v.push();
            v.set_preset(p);
            v.fit_points_3d(&pts, 0.08);
        });
        c.emit();
        return Ok(());
    }
    if c.view(|v| v.is_plan()) {
        c.view(|v| {
            v.yaw = PI / 4.0;
            v.pitch = (1.0 / 2f64.sqrt()).atan();
            if !pts.is_empty() {
                v.fit_points_3d(&pts, 0.08);
            }
        });
        c.emit();
    }
    c.dialog(DialogReq::Realtime("orbit")).await?;
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(
        dv("ZOOM", run(zoom))
            .aliases(&["Z"])
            .title("Zoom")
            .icon("zoom")
            .desc("Increases or decreases the magnification of the view in the current viewport."),
    );
    r.define(
        dv("PAN", run(pan))
            .aliases(&["P"])
            .title("Pan")
            .icon("pan")
            .desc("Shifts the view in the current viewport."),
    );
    r.define(
        dv("-PAN", run(pan_disp))
            .title("Pan by displacement")
            .icon("pan")
            .desc("Shifts the view by a specified displacement."),
    );
    r.define(
        dv("REGEN", run(regen))
            .aliases(&["RE"])
            .title("Regen")
            .desc("Regenerates the drawing and redraws the current viewport."),
    );
    r.define(
        dv("REGENALL", run(regen))
            .aliases(&["REA"])
            .title("Regen All")
            .desc("Regenerates the drawing and redraws all viewports."),
    );
    r.define(
        dv("REDRAW", run(redraw))
            .aliases(&["R"])
            .title("Redraw")
            .desc("Refreshes the display in the current viewport."),
    );
    let mut v = dv("-VIEW", run(named_view))
        .aliases(&["VIEW", "V"])
        .title("Named View")
        .icon("view")
        .desc("Saves and restores named views.");
    v.transparent = false;
    r.define(v);
    r.define(dv("3DORBIT", run(orbit)).aliases(&["3DO", "ORBIT"]).title("Orbit").icon("orbit").desc("Rotates the view in 3D space, but constrained to horizontal and vertical orbit only."));
}
