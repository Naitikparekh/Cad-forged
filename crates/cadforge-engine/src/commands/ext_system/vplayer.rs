//! VPLAYER, LAYVPI and LAYERPALL: layer settings that apply to one layout viewport only.
use super::{plural, ps};
use crate::commands::layer::match_layers;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::style::Color;

/// The layout whose viewports a command works on: the layout of the active viewport, else the current layout tab.
fn layout_name(c: &Ctx) -> Option<String> {
    c.ed(|e| e.mspace.as_ref().map(|m| m.layout.clone()))
        .or_else(|| c.doc(|d| (!d.is_model()).then(|| d.space.clone())))
}

fn active_viewport(c: &Ctx) -> Option<(String, u64)> {
    c.ed(|e| e.mspace.as_ref().map(|m| (m.layout.clone(), m.vp)))
}

fn viewports_of(c: &Ctx, layout: &str) -> Vec<u64> {
    c.doc(|d| {
        d.layout(layout)
            .map(|l| {
                l.entities
                    .iter()
                    .filter(|e| matches!(e.kind, Kind::Viewport(_)))
                    .map(|e| e.id)
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// Which viewports a VPLAYER option applies to: All / Select / Current.
async fn which_viewports(c: &Ctx) -> CmdResult<Option<(String, Vec<u64>)>> {
    let Some(layout) = layout_name(c) else {
        c.log_error("VPLAYER works on layout viewports: switch to a layout first.");
        return Ok(None);
    };
    let active = active_viewport(c);
    let default = if active.is_some() {
        "Current"
    } else {
        "Select"
    };
    let opts: &[&str] = if active.is_some() {
        &["All", "Select", "Current"]
    } else {
        &["All", "Select"]
    };
    let r = c
        .keyword(
            ps(&format!("Enter an option [{}]", opts.join("/")))
                .options(opts)
                .default_str(default)
                .allow_enter(),
        )
        .await?;
    let pick = r.kw().unwrap_or(default).to_string();
    let ids = match pick.as_str() {
        "All" => viewports_of(c, &layout),
        "Current" => active.map(|(_, id)| vec![id]).unwrap_or_default(),
        _ => {
            let sel = c
                .select(ps("Select viewports").filter(|e| matches!(e.kind, Kind::Viewport(_))))
                .await?;
            sel.ents
        }
    };
    if ids.is_empty() {
        c.log_error("No viewports were chosen.");
        return Ok(None);
    }
    Ok(Some((layout, ids)))
}

async fn ask_layers(c: &Ctx, prompt: &str) -> CmdResult<Vec<String>> {
    loop {
        let t = c.string(ps(prompt)).await?.text;
        if t.is_empty() {
            return Ok(vec![]);
        }
        let found = c.doc(|d| match_layers(d, &t));
        if found.is_empty() {
            c.log_error(&format!("No layer matched \"{t}\"."));
            continue;
        }
        return Ok(found);
    }
}

fn edit_all(c: &Ctx, layout: &str, ids: &[u64], f: impl Fn(&mut ViewportData)) -> usize {
    c.doc_mut(|d| {
        d.change(|d| {
            ids.iter()
                .filter(|id| d.edit_viewport(layout, **id, &f))
                .count()
        })
    })
}

async fn vplayer(c: Ctx, args: Args) -> CmdResult<()> {
    let opts = [
        "?",
        "Freeze",
        "Thaw",
        "Reset",
        "Newfrz",
        "Vpvisdflt",
        "Color",
        "Linetype",
        "LWeight",
        "Transparency",
    ];
    let option = match args.str("option") {
        Some(o) => o.to_string(),
        None => c
            .keyword(
                ps("Enter an option [?/Freeze/Thaw/Reset/Newfrz/Vpvisdflt]")
                    .options(&opts)
                    .allow_enter(),
            )
            .await?
            .kw()
            .unwrap_or("")
            .to_string(),
    };
    match option.as_str() {
        "?" => {
            let Some(layout) = layout_name(&c) else {
                c.log_error("VPLAYER works on layout viewports: switch to a layout first.");
                return Ok(());
            };
            let ids = match active_viewport(&c) {
                Some((_, id)) => vec![id],
                None => {
                    c.select(
                        ps("Select a viewport").filter(|e| matches!(e.kind, Kind::Viewport(_))),
                    )
                    .await?
                    .ents
                }
            };
            for id in ids {
                let (n, list) = c.doc(|d| {
                    let v = d
                        .layout(&layout)
                        .and_then(|l| l.entities.iter().find(|e| e.id == id))
                        .and_then(|e| {
                            if let Kind::Viewport(v) = &e.kind {
                                Some(v.clone())
                            } else {
                                None
                            }
                        });
                    (
                        v.as_ref().map_or(0, |v| v.layers.len()),
                        v.map(|v| v.layers).unwrap_or_default(),
                    )
                });
                if n == 0 {
                    c.log(&format!("Viewport {id}: no layer overrides."));
                }
                for o in list {
                    let mut what = vec![];
                    if o.frozen {
                        what.push("frozen".to_string());
                    }
                    if let Some(col) = o.color {
                        what.push(format!("colour {}", color_text(col)));
                    }
                    if let Some(lt) = &o.ltype {
                        what.push(format!("linetype {lt}"));
                    }
                    if let Some(lw) = o.lw {
                        what.push(format!("lineweight {lw}"));
                    }
                    if let Some(t) = o.transp {
                        what.push(format!("transparency {t}%"));
                    }
                    c.log(&format!("Viewport {id}: {}: {}", o.layer, what.join(", ")));
                }
            }
        }
        "Freeze" | "Thaw" | "Reset" => {
            let verb = option.to_lowercase();
            let layers = ask_layers(&c, &format!("Enter layer name(s) to {verb}")).await?;
            if layers.is_empty() {
                return Ok(());
            }
            let Some((layout, ids)) = which_viewports(&c).await? else {
                return Ok(());
            };
            let (freeze, reset) = (option == "Freeze", option == "Reset");
            let n = edit_all(&c, &layout, &ids, |v| {
                for l in &layers {
                    if reset {
                        v.reset_layer(l);
                    } else {
                        v.set_frozen(l, freeze);
                    }
                }
            });
            c.log(&format!(
                "{} in {}: {}.",
                plural(layers.len(), "layer"),
                plural(n, "viewport"),
                match option.as_str() {
                    "Freeze" => "frozen",
                    "Thaw" => "thawed",
                    _ => "reset to the drawing's settings",
                }
            ));
        }
        "Newfrz" => {
            let t = c
                .string(ps(
                    "Enter name(s) of new layer(s) that are frozen in all viewports",
                ))
                .await?
                .text;
            let names: Vec<String> = t
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if names.is_empty() {
                return Ok(());
            }
            let layout_names = c.doc(|d| d.layout_names());
            for name in &names {
                let created = c.doc_mut(|d| {
                    d.change(|d| {
                        if d.layer(name).is_none() && d.add_layer(name).is_err() {
                            return false;
                        }
                        if let Some(l) = d
                            .layers
                            .iter_mut()
                            .find(|l| l.name.eq_ignore_ascii_case(name))
                        {
                            l.vp_frozen_default = true;
                        }
                        true
                    })
                });
                if !created {
                    c.log_error(&format!("Layer \"{name}\" could not be created."));
                    continue;
                }
                for lay in &layout_names {
                    let ids = viewports_of(&c, lay);
                    edit_all(&c, lay, &ids, |v| v.set_frozen(name, true));
                }
            }
            c.log(&format!(
                "{} frozen in all viewports, existing and new.",
                plural(names.len(), "layer")
            ));
        }
        "Vpvisdflt" => {
            let layers = ask_layers(
                &c,
                "Enter layer name(s) to change the viewport visibility default",
            )
            .await?;
            if layers.is_empty() {
                return Ok(());
            }
            let frozen = c
                .keyword(
                    ps("Enter a viewport visibility default [Frozen/Thawed]")
                        .options(&["Frozen", "Thawed"])
                        .default_str("Thawed"),
                )
                .await?
                .kw()
                == Some("Frozen");
            let existing = c
                .keyword(
                    ps("Change visibility in current viewport(s)? [Yes/No]")
                        .options(&["Yes", "No"])
                        .default_str("No"),
                )
                .await?
                .kw()
                == Some("Yes");
            c.doc_mut(|d| {
                d.change(|d| {
                    for l in d
                        .layers
                        .iter_mut()
                        .filter(|l| layers.iter().any(|n| n.eq_ignore_ascii_case(&l.name)))
                    {
                        l.vp_frozen_default = frozen;
                    }
                })
            });
            if existing {
                if let Some(layout) = layout_name(&c) {
                    let ids = viewports_of(&c, &layout);
                    edit_all(&c, &layout, &ids, |v| {
                        for l in &layers {
                            v.set_frozen(l, frozen);
                        }
                    });
                }
            }
            c.log(&format!(
                "New viewports will show {} {}.",
                plural(layers.len(), "layer"),
                if frozen { "frozen" } else { "thawed" }
            ));
        }
        "Color" | "Linetype" | "LWeight" | "Transparency" => {
            // CadForge extension: the layer manager's "VP Color / VP Linetype / VP Lineweight / VP Transparency" columns
            let layers = ask_layers(&c, "Enter layer name(s)").await?;
            if layers.is_empty() {
                return Ok(());
            }
            let text = c
                .string(
                    ps(&format!(
                        "Enter the {} override, or Reset to remove it",
                        option.to_lowercase()
                    ))
                    .raw(),
                )
                .await?
                .text;
            let text = text.trim().to_string();
            let reset = text.eq_ignore_ascii_case("reset");
            let apply: Result<Box<dyn Fn(&mut LayerOverride)>, String> = match option.as_str() {
                "Color" => {
                    if reset {
                        Ok(Box::new(|o| o.color = None))
                    } else {
                        parse_color(&text).map(|col| {
                            Box::new(move |o: &mut LayerOverride| o.color = Some(col))
                                as Box<dyn Fn(&mut LayerOverride)>
                        })
                    }
                }
                "Linetype" => {
                    if reset {
                        Ok(Box::new(|o| o.ltype = None))
                    } else if cadforge_core::style::linetype(&text).is_some()
                        || text.eq_ignore_ascii_case("continuous")
                    {
                        let name = text.clone();
                        Ok(Box::new(move |o| o.ltype = Some(name.clone())))
                    } else {
                        Err(format!("Linetype \"{text}\" is not loaded."))
                    }
                }
                "LWeight" => {
                    if reset {
                        Ok(Box::new(|o| o.lw = None))
                    } else {
                        text.parse::<f64>()
                            .map(|mm| {
                                Box::new(move |o: &mut LayerOverride| {
                                    o.lw = Some((mm * 100.0).round() as i32)
                                })
                                    as Box<dyn Fn(&mut LayerOverride)>
                            })
                            .map_err(|_| {
                                "Enter a lineweight in millimetres, such as 0.35.".to_string()
                            })
                    }
                }
                _ => {
                    if reset {
                        Ok(Box::new(|o| o.transp = None))
                    } else {
                        text.parse::<f64>()
                            .ok()
                            .filter(|t| (0.0..=90.0).contains(t))
                            .map(|t| {
                                Box::new(move |o: &mut LayerOverride| o.transp = Some(t))
                                    as Box<dyn Fn(&mut LayerOverride)>
                            })
                            .ok_or_else(|| "Transparency is a percentage from 0 to 90.".to_string())
                    }
                }
            };
            let apply = match apply {
                Ok(f) => f,
                Err(e) => {
                    c.log_error(&e);
                    return Ok(());
                }
            };
            let Some((layout, ids)) = which_viewports(&c).await? else {
                return Ok(());
            };
            let n = edit_all(&c, &layout, &ids, |v| {
                for l in &layers {
                    v.edit_override(l, |o| apply(o));
                }
            });
            c.log(&format!(
                "{} overridden in {}.",
                plural(layers.len(), "layer"),
                plural(n, "viewport")
            ));
        }
        _ => {}
    }
    Ok(())
}

fn color_text(c: Color) -> String {
    match c {
        Color::Aci(n) => format!("{n}"),
        Color::Rgb(r, g, b) => format!("{r},{g},{b}"),
        Color::ByLayer => "ByLayer".into(),
        Color::ByBlock => "ByBlock".into(),
    }
}

fn parse_color(t: &str) -> Result<Color, String> {
    if let Ok(n) = t.parse::<u8>() {
        if (1..=255).contains(&n) {
            return Ok(Color::Aci(n));
        }
    }
    let parts: Vec<&str> = t.split(',').map(str::trim).collect();
    if parts.len() == 3 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            parts[0].parse::<u8>(),
            parts[1].parse::<u8>(),
            parts[2].parse::<u8>(),
        ) {
            return Ok(Color::Rgb(r, g, b));
        }
    }
    Err("Enter a colour number from 1 to 255 or red,green,blue.".into())
}

/// LAYVPI: in the active viewport, freeze every layer except those of the selected objects.
async fn layvpi(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some((layout, vp)) = active_viewport(&c) else {
        if c.doc(|d| d.is_model()) {
            c.log_error("LAYVPI is available only in a layout viewport.");
        } else {
            c.log_error("Activate a viewport first (double-click inside it, or MSPACE).");
        }
        return Ok(());
    };
    let sel = c
        .select(ps(
            "Select an object on the layer(s) to isolate in this viewport",
        ))
        .await?;
    let keep: std::collections::BTreeSet<String> = c
        .entities(&sel.ents)
        .iter()
        .map(|e| c.doc(|d| d.layer_of(e).name.to_lowercase()))
        .collect();
    if keep.is_empty() {
        return Ok(());
    }
    let all: Vec<String> = c.doc(|d| d.layers.iter().map(|l| l.name.clone()).collect());
    let cur = c.doc(|d| d.cur.layer.to_lowercase());
    edit_all(&c, &layout, &[vp], |v| {
        for l in &all {
            let lc = l.to_lowercase();
            if !keep.contains(&lc) && lc != cur {
                v.set_frozen(l, true);
            }
        }
    });
    c.log(&format!(
        "{} isolated in this viewport; the others are frozen here only.",
        plural(keep.len(), "layer")
    ));
    c.ed(|e| e.clear_sel());
    Ok(())
}

/// LAYERPALL: remove the colour / linetype / lineweight / transparency overrides in every viewport (VP freeze is kept;
/// use VPLAYER Reset for that).
async fn layerpall(c: Ctx, _a: Args) -> CmdResult<()> {
    let layouts = c.doc(|d| d.layout_names());
    let (mut vps, mut cleared) = (0, 0);
    for lay in &layouts {
        let ids = viewports_of(&c, lay);
        vps += ids.len();
        for id in ids {
            let n = c.doc(|d| {
                d.layout(lay)
                    .and_then(|l| l.entities.iter().find(|e| e.id == id))
                    .and_then(|e| {
                        if let Kind::Viewport(v) = &e.kind {
                            Some(
                                v.layers
                                    .iter()
                                    .filter(|o| {
                                        o.color.is_some()
                                            || o.ltype.is_some()
                                            || o.lw.is_some()
                                            || o.transp.is_some()
                                    })
                                    .count(),
                            )
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0)
            });
            if n > 0 {
                cleared += n;
                edit_all(&c, lay, &[id], |v| {
                    let names: Vec<String> = v.layers.iter().map(|o| o.layer.clone()).collect();
                    for name in names {
                        v.edit_override(&name, |o| {
                            o.color = None;
                            o.ltype = None;
                            o.lw = None;
                            o.transp = None;
                        });
                    }
                });
            }
        }
    }
    if vps == 0 {
        c.log_error("There are no layout viewports.");
    } else {
        c.log(&format!(
            "{} reset in {}.",
            plural(cleared, "layer override"),
            plural(vps, "viewport")
        ));
    }
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(def("Layers", "VPLAYER", run(vplayer)).title("Viewport Layer").icon("layers").desc("Freezes, thaws or resets layers in individual layout viewports, and sets defaults for new viewports."));
    r.define(
        def("Layers", "LAYVPI", run(layvpi))
            .title("Layer Viewport Isolate")
            .icon("layiso")
            .desc("Isolates the layer of a selected object in the active layout viewport."),
    );
    r.define(
        def("Layers", "LAYERPALL", run(layerpall))
            .title("Reset All Viewport Layer Overrides")
            .icon("layers")
            .desc("Resets the layer property overrides in all layout viewports."),
    );
}
