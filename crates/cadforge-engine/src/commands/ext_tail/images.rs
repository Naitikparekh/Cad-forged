//! Raster images: IMAGEATTACH, IMAGE, IMAGECLIP, IMAGEFRAME, IMAGEADJUST and IMAGEQUALITY.
//!
//! The picture file reaches the drawing through the host (file chooser) which registers it with
//! [`cadforge_core::drawing::Drawing::add_image`] and starts IMAGEATTACH with the picture's name.
use super::ps;
use crate::app::Event;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;

fn is_image(e: &Entity) -> bool {
    matches!(e.kind, Kind::Image(_))
}

fn image_of(c: &Ctx, id: u64) -> Option<ImageData> {
    match c.entity_by_id(id)?.kind {
        Kind::Image(i) => Some(i),
        _ => None,
    }
}

/// IMAGEATTACH: place a picture. Without a picture name the host is asked for a file first.
async fn imageattach(c: Ctx, args: Args) -> CmdResult<()> {
    let Some(name) = args.str("name").map(str::to_string) else {
        if c.app(|a| a.ui.dialogs) {
            c.app(|a| a.emit(Event::UiAction { name: "IMAGEATTACH".into(), args: Args::none() }));
        } else {
            c.log_error("IMAGEATTACH needs a picture: attach it with the file chooser (or Drawing::add_image).");
        }
        return Ok(());
    };
    let Some((px_w, px_h)) = c.doc(|d| d.images.get(&name).map(|i| (i.w as f64, i.h as f64))) else {
        c.log_error(&format!("Image \"{name}\" is not in this drawing."));
        return Ok(());
    };
    c.log(&format!("Attached {name} ({} x {} pixels).", px_w as u64, px_h as u64));
    let n0 = name.clone();
    c.set_preview(move |_, p| vec![PreviewItem::Entity(Entity::new(Kind::Image(ImageData::new(&n0, p, px_w, px_h, 0.0))))]);
    let r = c.point(ps("Specify insertion point").default_pt(Pt::ZERO)).await?;
    c.clear_preview();
    let Some(p) = r.pt() else { return Ok(()) };
    let n1 = name.clone();
    c.set_preview(move |_, q| {
        let s = dist(p, q).max(1e-9) / px_w;
        vec![PreviewItem::Entity(Entity::new(Kind::Image(ImageData::new(&n1, p, px_w * s, px_h * s, 0.0))))]
    });
    let k = c.distance(ps("Specify scale factor (drawing units per pixel) or [Unit]").options(&["Unit"]).base(p).default_num(1.0).nonzero()).await?;
    c.clear_preview();
    let scale = match k.value() {
        Some(v) => v,
        None => {
            if k.kw() == Some("Unit") {
                // "Unit": give the width of the whole picture in drawing units
                let Some(w) = c.distance(ps("Specify the width of the image in drawing units").base(p).nonzero()).await?.value() else { return Ok(()) };
                w / px_w
            } else {
                1.0
            }
        }
    };
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let rot = c.angle(ps("Specify rotation angle").base(p).default_num(0.0)).await?.value().unwrap_or(0.0);
    c.add(Entity::new(Kind::Image(ImageData::new(&name, p, px_w * scale, px_h * scale, rot))));
    Ok(())
}

/// IMAGE: list the pictures of the drawing or detach one.
async fn image_cmd(c: Ctx, _a: Args) -> CmdResult<()> {
    let rows: Vec<(String, String, u32, u32, usize, bool)> = c.doc(|d| {
        d.images
            .iter()
            .map(|(k, i)| {
                let uses = d.all_entities().filter(|e| matches!(&e.kind, Kind::Image(m) if &m.name == k)).count();
                (k.clone(), i.path.clone(), i.w, i.h, uses, i.is_missing())
            })
            .collect()
    });
    if rows.is_empty() {
        c.log("This drawing has no images. Use IMAGEATTACH to attach one.");
        return Ok(());
    }
    for (k, path, w, h, uses, missing) in &rows {
        c.log(&format!("{k}: {path}  {w} x {h} px  {uses} reference{}{}", if *uses == 1 { "" } else { "s" }, if *missing { "  (file not found)" } else { "" }));
    }
    let r = c.keyword(ps("Enter an option [Detach]").options(&["Detach"]).allow_enter()).await?;
    if r.kw() != Some("Detach") {
        return Ok(());
    }
    let n = c.string(ps("Enter the name of the image to detach")).await?.text;
    let Some((key, ..)) = rows.iter().find(|r| r.0.eq_ignore_ascii_case(n.trim())) else {
        c.log_error(&format!("Image \"{}\" not found.", n.trim()));
        return Ok(());
    };
    let key = key.clone();
    c.doc_mut(|d| {
        d.change(|d| {
            let ids: Vec<u64> = d.all_entities().filter(|e| matches!(&e.kind, Kind::Image(m) if m.name == key)).map(|e| e.id).collect();
            d.remove_all(&ids);
            d.images.shift_remove(&key);
        })
    });
    c.log(&format!("Image \"{key}\" detached."));
    c.emit();
    Ok(())
}

/// The point `q` in the picture's own frame: (0,0) at the lower-left corner, (1,1) at the opposite one.
fn to_local(im: &ImageData, q: Pt) -> Pt {
    let d = q - im.p;
    let det = im.u.x * im.v.y - im.u.y * im.v.x;
    if det.abs() < 1e-18 {
        return Pt::ZERO;
    }
    pt((d.x * im.v.y - d.y * im.v.x) / det, (im.u.x * d.y - im.u.y * d.x) / det)
}

fn from_local(im: &ImageData, s: f64, t: f64) -> Pt {
    im.p + im.u * s + im.v * t
}

async fn imageclip(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(id) = c.pick_entity(ps("Select image to clip").filter(is_image)).await? else { return Ok(()) };
    let Some(im) = image_of(&c, id) else { return Ok(()) };
    let has = !im.clip.is_empty();
    let r = c.keyword(ps("Enter image clipping option [ON/OFF/Delete/New boundary]").options(&["ON", "OFF", "Delete", "New boundary"]).default_str("New boundary")).await?;
    let kw = r.kw().unwrap_or("New boundary").to_string();
    let edit = |c: &Ctx, f: Box<dyn FnOnce(&mut ImageData)>| {
        c.doc_mut(|d| {
            d.modify(id, |e| {
                if let Kind::Image(i) = &mut e.kind {
                    f(i)
                }
            })
        });
        c.emit();
    };
    match kw.as_str() {
        "ON" | "OFF" => {
            if !has {
                c.log_error("The image has no clipping boundary yet. Use New boundary.");
                return Ok(());
            }
            let off = kw == "OFF";
            edit(&c, Box::new(move |i| i.clip_off = off));
        }
        "Delete" => edit(&c, Box::new(|i| {
            i.clip.clear();
            i.clip_off = false;
        })),
        _ => {
            let k = c.keyword(ps("Enter clipping type [Polygonal/Rectangular]").options(&["Polygonal", "Rectangular"]).default_str("Rectangular")).await?;
            let poly: Vec<Pt> = if k.kw() == Some("Polygonal") {
                let Some(first) = c.point(ps("Specify first point")).await?.pt() else { return Ok(()) };
                let mut pts = vec![first];
                loop {
                    let r = c.point(ps("Specify next point or [Undo]").options(&["Undo"]).base(*pts.last().expect("a point")).allow_enter()).await?;
                    match (r.kw(), r.pt()) {
                        (Some("Undo"), _) => {
                            if pts.len() > 1 {
                                pts.pop();
                            }
                        }
                        (_, Some(p)) => pts.push(p),
                        _ => break,
                    }
                }
                if pts.len() < 3 {
                    c.log_error("A clipping polygon needs at least three points.");
                    return Ok(());
                }
                pts
            } else {
                let Some(a) = c.point(ps("Specify first corner point")).await?.pt() else { return Ok(()) };
                let Some(b) = c.corner(a, ps("Specify opposite corner point")).await?.pt() else { return Ok(()) };
                // the rectangle is aligned with the picture, not with the screen
                let (la, lb) = (to_local(&im, a), to_local(&im, b));
                vec![from_local(&im, la.x, la.y), from_local(&im, lb.x, la.y), from_local(&im, lb.x, lb.y), from_local(&im, la.x, lb.y)]
            };
            edit(&c, Box::new(move |i| {
                i.clip = poly;
                i.clip_off = false;
            }));
        }
    }
    Ok(())
}

async fn imageframe(c: Ctx, args: Args) -> CmdResult<()> {
    let cur = c.doc(|d| d.settings.image_frame);
    let v = match args.num("value") {
        Some(v) => v,
        None => match c.number(ps("Enter image frame setting [0 = no frame, 1 = frame shown and plotted, 2 = frame shown, not plotted]").default_num(cur as f64)).await?.value() {
            Some(v) => v,
            None => return Ok(()),
        },
    };
    if !(0.0..=2.0).contains(&v) || v.fract() != 0.0 {
        c.log_error("Enter 0, 1 or 2.");
        return Ok(());
    }
    c.doc_mut(|d| d.change(|d| d.settings.image_frame = v as u8));
    c.emit();
    Ok(())
}

async fn imageadjust(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select image(s)").filter(is_image)).await?;
    let ids: Vec<u64> = sel.ents.iter().copied().filter(|i| image_of(&c, *i).is_some()).collect();
    let Some(first) = ids.first().and_then(|i| image_of(&c, *i)) else { return Ok(()) };
    let Some(b) = c.number(ps("Enter brightness value (0 = darkest, 100 = brightest)").default_num(first.brightness as f64).min(0.0)).await?.value() else { return Ok(()) };
    let Some(k) = c.number(ps("Enter contrast value (0 = lowest, 100 = highest)").default_num(first.contrast as f64).min(0.0)).await?.value() else { return Ok(()) };
    let Some(f) = c.number(ps("Enter fade value (0 = none, 100 = fully faded)").default_num(first.fade as f64).min(0.0)).await?.value() else { return Ok(()) };
    let (b, k, f) = (b.round().clamp(0.0, 100.0) as i32, k.round().clamp(0.0, 100.0) as i32, f.round().clamp(0.0, 100.0) as i32);
    c.doc_mut(|d| {
        d.change(|d| {
            for id in &ids {
                if let Some(Entity { kind: Kind::Image(i), .. }) = d.by_id_mut(*id) {
                    (i.brightness, i.contrast, i.fade) = (b, k, f);
                }
            }
        })
    });
    c.emit();
    Ok(())
}

async fn imagequality(c: Ctx, _a: Args) -> CmdResult<()> {
    let cur = if c.doc(|d| d.settings.image_draft) { "Draft" } else { "High" };
    let r = c.keyword(ps("Enter image quality setting [High/Draft]").options(&["High", "Draft"]).default_str(cur)).await?;
    if let Some(k) = r.kw() {
        c.doc_mut(|d| d.change(|d| d.settings.image_draft = k == "Draft"));
        c.emit();
    }
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(def("Blocks", "IMAGEATTACH", run(imageattach)).aliases(&["IAT", "IMAGEATT"]).title("Attach Image").icon("imageattach").desc("Inserts a reference to a raster image file (PNG or JPEG)."));
    r.define(def("Blocks", "IMAGE", run(image_cmd)).aliases(&["IM"]).title("Images").icon("imageattach").desc("Lists the images in the drawing and detaches them."));
    r.define(def("Blocks", "IMAGECLIP", run(imageclip)).aliases(&["ICL"]).title("Clip Image").icon("imageclip").desc("Crops the display of a selected image to a specified boundary."));
    r.define(def("Blocks", "IMAGEFRAME", run(imageframe)).title("Image Frame").icon("imageattach").desc("Controls whether image frames are shown and plotted.").transparent());
    r.define(def("Blocks", "IMAGEADJUST", run(imageadjust)).title("Adjust Image").icon("imageattach").desc("Controls the brightness, contrast and fade of the selected image."));
    r.define(def("Blocks", "IMAGEQUALITY", run(imagequality)).title("Image Quality").icon("imageattach").desc("Controls the display quality of images.").transparent());
}
