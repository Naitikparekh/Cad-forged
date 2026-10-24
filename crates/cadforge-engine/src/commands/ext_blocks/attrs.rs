//! Attribute commands: ATTDEF, ATTEDIT, EATTEDIT, ATTDISP, ATTSYNC and BATTMAN. A block's attribute definitions are
//! `Text` entities with `att` set; each block reference carries one `AttribData` per definition.
use super::{block_key, db, ps, ps_def};
use crate::commands::layer::wildcard_match;
use crate::commands::run;
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::drawing::{Block, Drawing};
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::style::AttDisp;
use cadforge_core::text::{AttDef, HJust, TextData, VJust};
use std::collections::HashMap;

// ---------------------------------------------------------------- model helpers

fn is_attdef(e: &Entity) -> bool {
    matches!(&e.kind, Kind::Text(t) if t.att.is_some())
}

fn has_attribs(e: &Entity) -> bool {
    matches!(&e.kind, Kind::Insert(i) if !i.attribs.is_empty())
}

/// The attribute definitions of a block, in definition order.
fn attdefs(b: &Block) -> Vec<TextData> {
    b.entities
        .iter()
        .filter_map(|e| match &e.kind {
            Kind::Text(t) if t.att.is_some() => Some(t.clone()),
            _ => None,
        })
        .collect()
}

fn att_of(t: &TextData) -> &AttDef {
    t.att.as_ref().expect("an attribute definition")
}

fn att_mut(t: &mut TextData) -> &mut AttDef {
    t.att.as_mut().expect("an attribute definition")
}

fn valid_tag(t: &str) -> bool {
    !t.is_empty() && !t.contains(|c: char| c.is_whitespace() || c == '!')
}

/// Tags of the constant attributes of `block`; their values live in the definition and cannot be edited.
fn constants(d: &Drawing, block: &str) -> Vec<String> {
    d.blocks
        .get(block)
        .map(|b| {
            attdefs(b)
                .iter()
                .map(att_of)
                .filter(|a| a.constant)
                .map(|a| a.tag.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// One attribute of a reference, placed in world space from its definition.
fn instance(def: &TextData, ins: &InsertData, base: Pt, value: &str, mode: AttDisp) -> AttribData {
    let a = att_of(def);
    let m = insert_matrix(ins, base);
    let k = mat::scale_factor(&m);
    let t = TextData {
        att: None,
        s: value.to_string(),
        p: mat::apply(&m, def.p),
        h: def.h * k,
        rot: def.rot + mat::angle(&m),
        w: def.w.map(|w| w * k),
        ..def.clone()
    };
    let hidden = match mode {
        AttDisp::On => false,
        AttDisp::Off => true,
        AttDisp::Normal => a.invisible,
    };
    AttribData {
        tag: a.tag.clone(),
        t,
        hidden,
    }
}

/// Rebuild the attributes of a reference from the block's definitions: values it already has are kept, new ones take
/// `values[tag]` or the definition's default, attributes the block no longer defines are dropped.
fn sync_attribs(
    ins: &mut InsertData,
    base: Pt,
    defs: &[TextData],
    mode: AttDisp,
    values: &HashMap<String, String>,
) {
    let mut old = std::mem::take(&mut ins.attribs);
    let rebuilt: Vec<AttribData> = defs
        .iter()
        .map(|d| {
            let a = att_of(d);
            let kept = if a.constant {
                None
            } else {
                old.iter()
                    .position(|o| o.tag == a.tag)
                    .map(|i| old.remove(i))
            };
            let value = kept
                .map(|o| o.t.s)
                .or_else(|| values.get(&a.tag).cloned())
                .unwrap_or_else(|| d.s.clone());
            instance(d, ins, base, &value, mode)
        })
        .collect();
    ins.attribs = rebuilt;
}

/// Bring every reference of block `name` (in all spaces and blocks) in line with its definitions.
fn sync_block(d: &mut Drawing, name: &str) -> usize {
    let Some(b) = d.blocks.get(name) else {
        return 0;
    };
    let (base, defs, mode) = (b.base, attdefs(b), d.settings.attdisp);
    d.change(|d| {
        let mut n = 0;
        for e in d.all_entities_mut() {
            if let Kind::Insert(i) = &mut e.kind {
                if i.name == name {
                    sync_attribs(i, base, &defs, mode, &HashMap::new());
                    n += 1;
                }
            }
        }
        n
    })
}

/// Apply an attribute display mode to the drawing and to every attribute in it.
fn set_attdisp(d: &mut Drawing, mode: AttDisp) {
    let defs: HashMap<String, Vec<TextData>> = d
        .blocks
        .iter()
        .map(|(n, b)| (n.clone(), attdefs(b)))
        .collect();
    d.change(|d| {
        d.settings.attdisp = mode;
        for e in d.all_entities_mut() {
            let Kind::Insert(InsertData { name, attribs, .. }) = &mut e.kind else {
                continue;
            };
            for a in attribs.iter_mut() {
                let invisible = defs
                    .get(name.as_str())
                    .and_then(|v| v.iter().map(att_of).find(|x| x.tag == a.tag))
                    .map(|x| x.invisible);
                a.hidden = match mode {
                    AttDisp::On => false,
                    AttDisp::Off => true,
                    AttDisp::Normal => invisible.unwrap_or(a.hidden),
                };
            }
        }
    });
}

/// Attribute `i` of reference `id`, with the reference's block name.
fn attrib(c: &Ctx, id: u64, i: usize) -> Option<(String, AttribData)> {
    c.doc(|d| match d.by_id(id).map(|e| &e.kind) {
        Some(Kind::Insert(n)) => n.attribs.get(i).map(|a| (n.name.clone(), a.clone())),
        _ => None,
    })
}

/// Change attribute `i` of reference `id`.
fn edit(c: &Ctx, id: u64, i: usize, f: impl FnOnce(&mut AttribData)) {
    c.doc_mut(|d| {
        d.change(|d| {
            if let Some(Kind::Insert(n)) = d.by_id_mut(id).map(|e| &mut e.kind) {
                if let Some(a) = n.attribs.get_mut(i) {
                    f(a);
                }
            }
        })
    });
}

/// Replace `old` by `new` in `value`; an empty `old` replaces the whole value.
fn modify(value: &str, old: &str, new: &str) -> String {
    if old.is_empty() {
        new.to_string()
    } else {
        value.replace(old, new)
    }
}

// ---------------------------------------------------------------- attribute values for new references

async fn ask_value(c: &Ctx, def: &TextData) -> CmdResult<String> {
    let a = att_of(def);
    let label = if a.prompt.is_empty() {
        a.tag.as_str()
    } else {
        a.prompt.as_str()
    }
    .trim_end_matches(':');
    let mut v = c.string(ps_def(label, &def.s)).await?.text;
    if a.verify {
        v = c.string(ps_def(&format!("Verify {label}"), &v)).await?.text;
    }
    Ok(v)
}

/// Give the block reference `id` the attributes its block defines. With `ask` the value of each attribute is prompted
/// for, as the definition's prompt with its default (constant and preset attributes take the default silently);
/// otherwise every attribute takes its default. Attributes the reference already has are kept. INSERT and BLOCK call
/// this after creating a reference; external references have no attributes of their own.
pub async fn fill_attributes(c: &Ctx, id: u64, ask: bool) -> CmdResult<()> {
    let Some((base, defs, missing)) = c.doc(|d| match d.by_id(id).map(|e| &e.kind) {
        Some(Kind::Insert(i)) => d.blocks.get(&i.name).filter(|b| b.xref.is_none()).map(|b| {
            let defs = attdefs(b);
            let missing: Vec<TextData> = defs
                .iter()
                .filter(|t| !i.attribs.iter().any(|a| a.tag == att_of(t).tag))
                .cloned()
                .collect();
            (b.base, defs, missing)
        }),
        _ => None,
    }) else {
        return Ok(());
    };
    let mut values = HashMap::new();
    for t in missing
        .iter()
        .filter(|t| ask && !att_of(t).constant && !att_of(t).preset)
    {
        values.insert(att_of(t).tag.clone(), ask_value(c, t).await?);
    }
    if !missing.is_empty() {
        c.doc_mut(|d| {
            let mode = d.settings.attdisp;
            d.change(|d| {
                if let Some(Kind::Insert(i)) = d.by_id_mut(id).map(|e| &mut e.kind) {
                    sync_attribs(i, base, &defs, mode, &values);
                }
            })
        });
    }
    Ok(())
}

// ---------------------------------------------------------------- ATTDEF

const MODES: [&str; 4] = ["Invisible", "Constant", "Verify", "Preset"];
const JUSTIFY: [&str; 13] = [
    "Left", "Center", "Right", "Middle", "TL", "TC", "TR", "ML", "MC", "MR", "BL", "BC", "BR",
];

fn justification(k: &str) -> (HJust, VJust) {
    use HJust::*;
    use VJust::*;
    match k {
        "Center" => (Center, Baseline),
        "Right" => (Right, Baseline),
        "Middle" | "MC" => (Center, Middle),
        "TL" => (Left, Top),
        "TC" => (Center, Top),
        "TR" => (Right, Top),
        "ML" => (Left, Middle),
        "MR" => (Right, Middle),
        "BL" => (Left, Bottom),
        "BC" => (Center, Bottom),
        "BR" => (Right, Bottom),
        _ => (Left, Baseline),
    }
}

async fn attdef(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut mode = [false; 4];
    loop {
        let yn = |b: bool| if b { 'Y' } else { 'N' };
        c.log(&format!(
            "Current attribute modes:  Invisible={} Constant={} Verify={} Preset={}",
            yn(mode[0]),
            yn(mode[1]),
            yn(mode[2]),
            yn(mode[3])
        ));
        let k = c
            .keyword(
                ps("Enter an option to change")
                    .options(&MODES)
                    .default_str("done"),
            )
            .await?;
        match MODES.iter().position(|m| Some(*m) == k.kw()) {
            Some(i) => mode[i] = !mode[i],
            None => break,
        }
    }
    let tag = c
        .string(ps("Enter attribute tag name"))
        .await?
        .text
        .to_uppercase();
    if tag.is_empty() {
        return Ok(());
    }
    if !valid_tag(&tag) {
        c.log_error("Attribute tags cannot contain spaces or exclamation points.");
        return Ok(());
    }
    let (prompt, value) = if mode[1] {
        (
            String::new(),
            c.string(ps("Enter attribute value")).await?.text,
        )
    } else {
        (
            c.string(ps("Enter attribute prompt")).await?.text,
            c.string(ps("Enter default attribute value")).await?.text,
        )
    };
    let (mut jh, mut jv) = (HJust::Left, VJust::Baseline);
    let p = loop {
        let r = c
            .point(ps("Specify start point of text").options(&["Justify"]))
            .await?;
        if r.kw() == Some("Justify") {
            let j = c
                .keyword(ps("Enter an option").options(&JUSTIFY).default_str("Left"))
                .await?;
            (jh, jv) = justification(j.kw().unwrap_or("Left"));
            continue;
        }
        match r.pt() {
            Some(p) => break p,
            None => return Ok(()),
        }
    };
    let mem_h = c.app(|a| a.mem.text_height);
    let Some(h) = c
        .distance(
            ps("Specify height")
                .base(p)
                .default_num(mem_h)
                .min(0.0)
                .nonzero(),
        )
        .await?
        .value()
    else {
        return Ok(());
    };
    c.app(|a| a.mem.text_height = h);
    let Some(rot) = c
        .angle(
            ps("Specify rotation angle of text")
                .base(p)
                .default_num(0.0),
        )
        .await?
        .value()
    else {
        return Ok(());
    };
    let mut t = TextData::new(p, h, &value).with_rot(rot).with_just(jh, jv);
    t.att = Some(AttDef {
        tag,
        prompt,
        invisible: mode[0],
        constant: mode[1],
        verify: mode[2],
        preset: mode[3],
    });
    c.add(Entity::new(Kind::Text(t)));
    Ok(())
}

// ---------------------------------------------------------------- ATTEDIT / EATTEDIT

/// Attributes (reference id, index) of the references `ids` whose block name, tag and value match the wildcard
/// specifications; constant attributes are left out.
fn matching(d: &Drawing, ids: &[u64], spec: &[String], visible_only: bool) -> Vec<(u64, usize)> {
    let mut out = vec![];
    for &id in ids {
        let Some(Kind::Insert(ins)) = d.by_id(id).map(|e| &e.kind) else {
            continue;
        };
        if !wildcard_match(&spec[0], &ins.name) {
            continue;
        }
        let constant = constants(d, &ins.name);
        for (i, a) in ins.attribs.iter().enumerate() {
            if wildcard_match(&spec[1], &a.tag)
                && wildcard_match(&spec[2], &a.t.s)
                && !(visible_only && a.hidden)
                && !constant.contains(&a.tag)
            {
                out.push((id, i));
            }
        }
    }
    out
}

/// Edit one attribute: its value, position, height and rotation, until Next.
async fn edit_one(c: &Ctx, id: u64, i: usize) -> CmdResult<()> {
    c.ed(|e| e.temp_sel = [id].into_iter().collect());
    if let Some((block, a)) = attrib(c, id, i) {
        c.log(&format!(
            "Block \"{block}\", attribute {} = \"{}\"",
            a.tag, a.t.s
        ));
    }
    loop {
        let Some((_, a)) = attrib(c, id, i) else {
            return Ok(());
        };
        let k = c
            .keyword(
                ps("Enter an option")
                    .options(&["Value", "Position", "Height", "Angle", "Next"])
                    .default_str("N"),
            )
            .await?;
        match k.kw() {
            Some("Value") => {
                let m = c
                    .keyword(
                        ps("Enter type of value modification")
                            .options(&["Modify", "Replace"])
                            .default_str("R"),
                    )
                    .await?;
                let value = if m.kw() == Some("Modify") {
                    let old = c.string(ps("Enter string to change")).await?.text;
                    let new = c.string(ps("Enter new string")).await?.text;
                    modify(&a.t.s, &old, &new)
                } else {
                    c.string(ps("Enter new attribute value")).await?.text
                };
                edit(c, id, i, |a| a.t.s = value);
            }
            Some("Position") => {
                if let Some(p) = c
                    .point(ps("Specify new text insertion point").default_pt(a.t.p))
                    .await?
                    .pt()
                {
                    edit(c, id, i, |a| a.t.p = p);
                }
            }
            Some("Height") => {
                if let Some(h) = c
                    .distance(
                        ps("Specify new height")
                            .base(a.t.p)
                            .default_num(a.t.h)
                            .min(0.0)
                            .nonzero(),
                    )
                    .await?
                    .value()
                {
                    edit(c, id, i, |a| a.t.h = h);
                }
            }
            Some("Angle") => {
                if let Some(r) = c
                    .angle(
                        ps("Specify new rotation angle")
                            .base(a.t.p)
                            .default_num(a.t.rot),
                    )
                    .await?
                    .value()
                {
                    edit(c, id, i, |a| a.t.rot = r);
                }
            }
            _ => return Ok(()),
        }
    }
}

async fn attedit(c: Ctx, _a: Args) -> CmdResult<()> {
    let one = c
        .keyword(
            ps("Edit attributes one at a time?")
                .options(&["Yes", "No"])
                .default_str("Y"),
        )
        .await?
        .kw()
        != Some("No");
    let visible_only = !one
        && c.keyword(
            ps("Edit only attributes visible on screen?")
                .options(&["Yes", "No"])
                .default_str("Y"),
        )
        .await?
        .kw()
            != Some("No");
    let mut spec = vec![];
    for what in ["block name", "attribute tag", "attribute value"] {
        let s = c
            .string(ps(&format!("Enter {what} specification")).default_str("*"))
            .await?
            .text;
        spec.push(if s.is_empty() { "*".to_string() } else { s });
    }
    let sel = c
        .select(ps("Select Attributes").filter(has_attribs))
        .await?;
    let targets = c.doc(|d| matching(d, &sel.ents, &spec, visible_only));
    c.log(&format!(
        "{} attribute{} selected.",
        targets.len(),
        if targets.len() == 1 { "" } else { "s" }
    ));
    if targets.is_empty() {
        return Ok(());
    }
    if one {
        for (id, i) in targets {
            edit_one(&c, id, i).await?;
        }
        c.ed(|e| e.temp_sel.clear());
        return Ok(());
    }
    let old = c.string(ps("Enter string to change")).await?.text;
    let new = c.string(ps("Enter new string")).await?.text;
    for (id, i) in targets {
        edit(&c, id, i, |a| a.t.s = modify(&a.t.s, &old, &new));
    }
    Ok(())
}

/// (tag, prompt, value, constant) of each attribute of reference `id`.
fn attribute_rows(d: &Drawing, id: u64) -> Vec<(String, String, String, bool)> {
    let Some(Kind::Insert(ins)) = d.by_id(id).map(|e| &e.kind) else {
        return vec![];
    };
    let defs = d.blocks.get(&ins.name).map(attdefs).unwrap_or_default();
    ins.attribs
        .iter()
        .map(|a| {
            let def = defs.iter().map(att_of).find(|x| x.tag == a.tag);
            (
                a.tag.clone(),
                def.map(|x| x.prompt.clone()).unwrap_or_default(),
                a.t.s.clone(),
                def.is_some_and(|x| x.constant),
            )
        })
        .collect()
}

async fn eattedit(c: Ctx, args: Args) -> CmdResult<()> {
    let id = match args.id("entity") {
        Some(id) => id,
        None => match c
            .pick_entity(ps("Select a block").filter(has_attribs))
            .await?
        {
            Some(id) => id,
            None => return Ok(()),
        },
    };
    c.ed(|e| e.temp_sel = [id].into_iter().collect());
    loop {
        let rows = c.doc(|d| attribute_rows(d, id));
        for (tag, prompt, value, constant) in &rows {
            c.log(&format!(
                "{tag:<12} {prompt:<24} {value}{}",
                if *constant { "  (constant)" } else { "" }
            ));
        }
        let t = c
            .string(ps("Enter attribute tag to edit or [All] <exit>"))
            .await?
            .text;
        let picked: Vec<usize> = if t.is_empty() {
            return Ok(());
        } else if let Some(i) = rows.iter().position(|r| r.0.eq_ignore_ascii_case(&t)) {
            vec![i]
        } else if t.eq_ignore_ascii_case("all") {
            (0..rows.len()).collect()
        } else {
            c.log_error(&format!("No attribute with tag \"{t}\"."));
            continue;
        };
        for i in picked {
            let (tag, _, value, constant) = &rows[i];
            if *constant {
                c.log_error(&format!(
                    "{tag} is a constant attribute and cannot be edited."
                ));
                continue;
            }
            let v = c
                .string(ps_def(&format!("Enter new value for {tag}"), value))
                .await?
                .text;
            edit(&c, id, i, |a| a.t.s = v);
        }
    }
}

// ---------------------------------------------------------------- ATTDISP / ATTSYNC

async fn attdisp(c: Ctx, _a: Args) -> CmdResult<()> {
    let name = |m: AttDisp| match m {
        AttDisp::Normal => "Normal",
        AttDisp::On => "ON",
        AttDisp::Off => "OFF",
    };
    let cur = c.doc(|d| d.settings.attdisp);
    let k = c
        .keyword(
            ps("Enter attribute visibility setting")
                .options(&["Normal", "ON", "OFF"])
                .default_str(name(cur)),
        )
        .await?;
    let mode = match k.kw() {
        Some("Normal") => AttDisp::Normal,
        Some("ON") => AttDisp::On,
        Some("OFF") => AttDisp::Off,
        _ => return Ok(()),
    };
    c.doc_mut(|d| set_attdisp(d, mode));
    c.emit();
    Ok(())
}

/// Log every block that defines attributes with its definition and reference counts.
fn list_attribute_blocks(c: &Ctx) {
    let lines: Vec<String> = c.doc(|d| {
        d.blocks
            .iter()
            .filter_map(|(n, b)| {
                let defs = attdefs(b).len();
                let refs = d
                    .all_entities()
                    .filter(|e| matches!(&e.kind, Kind::Insert(i) if i.name == *n))
                    .count();
                (defs > 0).then(|| {
                    format!(
                        "{n}: {defs} attribute definition{}, {refs} reference{}",
                        if defs == 1 { "" } else { "s" },
                        if refs == 1 { "" } else { "s" }
                    )
                })
            })
            .collect()
    });
    c.log(&if lines.is_empty() {
        "No blocks with attributes.".to_string()
    } else {
        lines.join("\n")
    });
}

async fn attsync(c: Ctx, _a: Args) -> CmdResult<()> {
    let name = loop {
        let k = c
            .keyword(
                ps("Enter an option")
                    .options(&["?", "Name", "Select"])
                    .default_str("Select"),
            )
            .await?;
        match k.kw() {
            Some("?") => list_attribute_blocks(&c),
            Some("Name") => {
                let t = c
                    .string(ps("Enter name of block to synchronize or [?]"))
                    .await?
                    .text;
                match t.as_str() {
                    "" => return Ok(()),
                    "?" => list_attribute_blocks(&c),
                    _ => match block_key(&c, &t) {
                        Some(n) => break n,
                        None => {
                            c.log_error(&format!("Block \"{t}\" not found."));
                            return Ok(());
                        }
                    },
                }
            }
            _ => {
                let Some(id) = c
                    .pick_entity(
                        ps("Select a block reference")
                            .filter(|e| matches!(e.kind, Kind::Insert(_))),
                    )
                    .await?
                else {
                    return Ok(());
                };
                if let Some(Kind::Insert(i)) = c.entity_by_id(id).map(|e| e.kind) {
                    break i.name;
                }
            }
        }
    };
    let n = c.doc_mut(|d| sync_block(d, &name));
    c.log(&format!(
        "Block \"{name}\" synchronized: {n} reference{} updated.",
        if n == 1 { "" } else { "s" }
    ));
    Ok(())
}

// ---------------------------------------------------------------- BATTMAN

/// Change the `k`th attribute definition of `block`.
fn set_def(c: &Ctx, block: &str, k: usize, f: impl FnOnce(&mut TextData)) {
    c.doc_mut(|d| {
        d.change(|d| {
            let Some(b) = d.blocks.get_mut(block) else {
                return;
            };
            if let Some(Kind::Text(t)) = b
                .entities
                .iter_mut()
                .filter(|e| is_attdef(e))
                .nth(k)
                .map(|e| &mut e.kind)
            {
                f(t);
            }
        })
    });
}

/// Move (`Some(up)`) or remove (`None`) the `k`th attribute definition of `block`.
fn arrange_def(c: &Ctx, block: &str, k: usize, up: Option<bool>) {
    c.doc_mut(|d| {
        d.change(|d| {
            let Some(b) = d.blocks.get_mut(block) else {
                return;
            };
            let at: Vec<usize> = b
                .entities
                .iter()
                .enumerate()
                .filter(|(_, e)| is_attdef(e))
                .map(|(i, _)| i)
                .collect();
            match up {
                None => {
                    b.entities.remove(at[k]);
                }
                Some(up) => {
                    let to = if up { k.checked_sub(1) } else { Some(k + 1) };
                    if let Some(j) = to.filter(|j| *j < at.len()) {
                        b.entities.swap(at[k], at[j]);
                    }
                }
            }
        })
    });
}

/// Change the properties of the `k`th attribute definition until done.
async fn edit_def(c: &Ctx, block: &str, k: usize) -> CmdResult<()> {
    let props = [
        "Tag",
        "Prompt",
        "Default",
        "Invisible",
        "Constant",
        "Verify",
        "Preset",
    ];
    loop {
        let r = c
            .keyword(
                ps("Enter attribute property to change")
                    .options(&props)
                    .default_str("done"),
            )
            .await?;
        match r.kw() {
            Some("Tag") => {
                let t = c.string(ps("Enter new tag")).await?.text.to_uppercase();
                if valid_tag(&t) {
                    set_def(c, block, k, |x| att_mut(x).tag = t);
                } else {
                    c.log_error("Attribute tags cannot contain spaces or exclamation points.");
                }
            }
            Some("Prompt") => {
                let p = c.string(ps("Enter new prompt")).await?.text;
                set_def(c, block, k, |x| att_mut(x).prompt = p);
            }
            Some("Default") => {
                let v = c.string(ps("Enter new default value")).await?.text;
                set_def(c, block, k, |x| x.s = v);
            }
            Some(m @ ("Invisible" | "Constant" | "Verify" | "Preset")) => {
                set_def(c, block, k, |x| {
                    let a = att_mut(x);
                    match m {
                        "Invisible" => a.invisible = !a.invisible,
                        "Constant" => a.constant = !a.constant,
                        "Verify" => a.verify = !a.verify,
                        _ => a.preset = !a.preset,
                    }
                })
            }
            _ => return Ok(()),
        }
    }
}

async fn battman(c: Ctx, _a: Args) -> CmdResult<()> {
    let r = c
        .entity(
            ps("Select a block reference")
                .options(&["Name"])
                .filter(|e| matches!(e.kind, Kind::Insert(_))),
        )
        .await?;
    let name = match r {
        EntR::Entity { id, .. } => match c.entity_by_id(id).map(|e| e.kind) {
            Some(Kind::Insert(i)) => i.name,
            _ => return Ok(()),
        },
        EntR::Kw(_) => {
            let t = c.string(ps("Enter block name")).await?.text;
            match block_key(&c, &t) {
                Some(n) => n,
                None => {
                    c.log_error(&format!("Block \"{t}\" not found."));
                    return Ok(());
                }
            }
        }
        EntR::Enter => return Ok(()),
    };
    let mut changed = false;
    loop {
        let defs = c.doc(|d| d.blocks.get(&name).map(attdefs).unwrap_or_default());
        if defs.is_empty() {
            c.log_error(&format!("Block \"{name}\" has no attribute definitions."));
            break;
        }
        c.log(&format!("Attributes of block \"{name}\":"));
        for (n, t) in defs.iter().enumerate() {
            let a = att_of(t);
            let flags: Vec<&str> = MODES
                .iter()
                .zip([a.invisible, a.constant, a.verify, a.preset])
                .filter(|(_, on)| *on)
                .map(|(m, _)| *m)
                .collect();
            c.log(&format!(
                "{}. {:<12} {:<24} {:<12} {}",
                n + 1,
                a.tag,
                a.prompt,
                t.s,
                flags.join(" ")
            ));
        }
        let k = c
            .keyword(
                ps("Enter an option")
                    .options(&["Edit", "Remove", "Up", "Down", "Sync", "eXit"])
                    .default_str("eXit"),
            )
            .await?;
        let op = match k.kw() {
            Some(op @ ("Edit" | "Remove" | "Up" | "Down")) => op,
            Some("Sync") => {
                let n = c.doc_mut(|d| sync_block(d, &name));
                c.log(&format!(
                    "{n} reference{} synchronized.",
                    if n == 1 { "" } else { "s" }
                ));
                changed = false;
                continue;
            }
            _ => break,
        };
        let tag = c.string(ps("Enter attribute tag")).await?.text;
        let Some(i) = defs
            .iter()
            .position(|t| att_of(t).tag.eq_ignore_ascii_case(&tag))
        else {
            c.log_error(&format!("No attribute with tag \"{tag}\"."));
            continue;
        };
        match op {
            "Edit" => edit_def(&c, &name, i).await?,
            "Remove" => arrange_def(&c, &name, i, None),
            _ => arrange_def(&c, &name, i, Some(op == "Up")),
        }
        changed = true;
    }
    if changed {
        let n = c.doc_mut(|d| sync_block(d, &name));
        c.log(&format!(
            "{n} reference{} updated.",
            if n == 1 { "" } else { "s" }
        ));
    }
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(
        db("ATTDEF", run(attdef))
            .aliases(&["ATT", "-ATT", "-ATTDEF"])
            .title("Define Attributes")
            .icon("text")
            .desc("Creates an attribute definition for storing data in a block."),
    );
    r.define(
        db("ATTEDIT", run(attedit))
            .aliases(&["ATE", "-ATE", "-ATTEDIT"])
            .title("Edit Attributes")
            .icon("textedit")
            .desc("Changes attribute information in a block."),
    );
    r.define(
        db("EATTEDIT", run(eattedit))
            .title("Edit Block Attributes")
            .icon("textedit")
            .desc("Edits the attribute values of a block reference."),
    );
    r.define(
        db("ATTDISP", run(attdisp))
            .title("Attribute Display")
            .icon("block")
            .desc("Globally controls attribute visibility."),
    );
    r.define(
        db("ATTSYNC", run(attsync))
            .title("Synchronize Attributes")
            .icon("block")
            .desc("Updates block references with the attributes currently defined for the block."),
    );
    r.define(
        db("BATTMAN", run(battman))
            .title("Block Attribute Manager")
            .icon("block")
            .desc("Manages the attributes of a block definition."),
    );
}
