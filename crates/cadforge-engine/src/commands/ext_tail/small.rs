//! Small annotation commands: MTEXTEDIT, TEXTALIGN, TCOUNT, PASTESPEC and QTEXT.
use super::ps;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;

fn is_text(e: &Entity) -> bool {
    matches!(e.kind, Kind::Text(_))
}

/// MTEXTEDIT: edit multiline text in the in-place editor (TEXTEDIT, limited to multiline text).
async fn mtextedit(c: Ctx, a: Args) -> CmdResult<()> {
    let id = match a.id("entity") {
        Some(i) => Some(i),
        None => c.entity(ps("Select an MTEXT object").filter(|e| matches!(&e.kind, Kind::Text(t) if t.multi))).await?.id(),
    };
    let Some(id) = id else { return Ok(()) };
    c.sub_forced("TEXTEDIT", Args::none().with("entity", ArgVal::Id(id))).await;
    Ok(())
}

/// TEXTALIGN: move text objects (perpendicular to a line) until their insertion points lie on the line.
async fn textalign(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select text objects to align").filter(is_text)).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let Some(from) = c.point(ps("Specify a point on the alignment line")).await?.pt() else { return Ok(()) };
    let to = c.point(ps("Specify the direction of the line or press Enter for horizontal").base(from).allow_enter()).await?;
    let u = match to.pt() {
        Some(b) if dist(from, b) > 1e-9 => unit(b - from),
        _ => pt(1.0, 0.0),
    };
    let n = pt(-u.y, u.x);
    let ids = sel.ents.clone();
    let moved = c.doc_mut(|d| {
        d.change(|d| {
            let mut moved = 0;
            for id in ids {
                if let Some(Entity { kind: Kind::Text(t), .. }) = d.by_id_mut(id) {
                    let off = dot(t.p - from, n);
                    if off.abs() > 1e-12 {
                        t.p = t.p - n * off;
                        moved += 1;
                    }
                }
            }
            moved
        })
    });
    c.log(&format!("{moved} text object{} aligned.", if moved == 1 { "" } else { "s" }));
    c.ed(|e| e.clear_sel());
    Ok(())
}

fn num_text(v: f64) -> String {
    if v.fract().abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// TCOUNT: number text objects in the order they were selected (overwrite, or add before / after the old text).
async fn tcount(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select text objects to number").filter(is_text)).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let Some(start) = c.number(ps("Specify the starting number").default_num(1.0)).await?.value() else { return Ok(()) };
    let Some(step) = c.number(ps("Specify the increment").default_num(1.0)).await?.value() else { return Ok(()) };
    let how = c.keyword(ps("Placement [Overwrite/Prefix/Suffix]").options(&["Overwrite", "Prefix", "Suffix"]).default_str("Overwrite")).await?;
    let how = how.kw().unwrap_or("Overwrite").to_string();
    let ids = sel.ents.clone();
    c.doc_mut(|d| {
        d.change(|d| {
            for (i, id) in ids.iter().enumerate() {
                if let Some(Entity { kind: Kind::Text(t), .. }) = d.by_id_mut(*id) {
                    let n = num_text(start + step * i as f64);
                    t.s = match how.as_str() {
                        "Prefix" => format!("{n}{}", t.s),
                        "Suffix" => format!("{}{n}", t.s),
                        _ => n,
                    };
                }
            }
        })
    });
    c.log(&format!("{} numbered from {} by {}.", super::plural(ids.len(), "text object"), num_text(start), num_text(step)));
    c.ed(|e| e.clear_sel());
    Ok(())
}

/// PASTESPEC: choose how the clipboard is pasted.
async fn pastespec(c: Ctx, a: Args) -> CmdResult<()> {
    let how = match a.str("option") {
        Some(o) => o.to_string(),
        None => c.keyword(ps("Paste as [Paste/Block/Original coordinates]").options(&["Paste", "Block", "Original coordinates"]).default_str("Paste")).await?.kw().unwrap_or("Paste").to_string(),
    };
    let cmd = match how.as_str() {
        "Block" => "PASTEBLOCK",
        "Original coordinates" => "PASTEORIG",
        _ => "PASTECLIP",
    };
    c.sub_forced(cmd, Args::none()).await;
    Ok(())
}

/// QTEXT: show text as boxes while drawing is slow or crowded.
async fn qtext(c: Ctx, _a: Args) -> CmdResult<()> {
    let cur = c.pref("QTEXTMODE") != 0.0;
    let r = c.keyword(ps("Enter mode [ON/OFF]").options(&["ON", "OFF"]).default_str(if cur { "ON" } else { "OFF" })).await?;
    let on = r.kw() == Some("ON");
    c.setvar("QTEXTMODE", if on { 1.0 } else { 0.0 })?;
    c.log(&format!("Quick text is {}.", if on { "on: text is drawn as boxes" } else { "off" }));
    c.emit();
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(def("Annotate", "MTEXTEDIT", run(mtextedit)).title("Edit Multiline Text").icon("mtext").desc("Edits multiline text in the in-place editor."));
    r.define(def("Annotate", "TEXTALIGN", run(textalign)).title("Align Text").icon("text").desc("Moves text objects onto a line, perpendicular to it."));
    r.define(def("Annotate", "TCOUNT", run(tcount)).title("Number Text").icon("text").desc("Numbers text objects in selection order with a start and an increment."));
    r.define(def("File", "PASTESPEC", run(pastespec)).title("Paste Special").icon("paste").desc("Pastes the clipboard as objects, as a block, or at its original coordinates."));
    r.define(def("Annotate", "QTEXT", run(qtext)).title("Quick Text").icon("text").desc("Draws text as boxes (QTEXTMODE).").transparent());
}
