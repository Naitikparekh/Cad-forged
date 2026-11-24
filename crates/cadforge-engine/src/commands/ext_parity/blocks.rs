//! Block tools: COUNT, COUNTLIST and COUNTTABLE count block references (and other identical objects), BURST explodes
//! block references keeping the attribute values as text, NCOPY copies objects out of blocks and external references,
//! BLOCKREPLACE swaps every reference of one block for another.
use super::{plural, ps};
use crate::commands::helpers::pv;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use std::collections::BTreeMap;

// ---------------------------------------------------------------- COUNT

/// What makes two objects "the same" for COUNT: the block name of a block reference; for anything else its type and
/// size (the extents and the length of its outline, which do not change when it is moved).
fn count_key(e: &Entity, d: &Drawing) -> String {
    if let Kind::Insert(i) = &e.kind {
        return format!("block:{}", i.name.to_ascii_uppercase());
    }
    let r = |v: f64| (v * 1e6).round() / 1e6;
    let (w, h) = bounds_of(e, d).map(|b| (r(b.max_x - b.min_x), r(b.max_y - b.min_y))).unwrap_or_default();
    let len: f64 = segs_of(e, d).iter().map(|s| s.length()).sum();
    let text = match &e.kind {
        Kind::Text(t) => t.s.clone(),
        _ => String::new(),
    };
    format!("{}:{w}:{h}:{}:{text}", e.type_name(), r(len))
}

/// The block references of the current space, counted by block name (sorted by name, case-insensitively).
fn block_counts(d: &Drawing) -> Vec<(String, usize)> {
    let mut m: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for e in d.entities() {
        if let Kind::Insert(i) = &e.kind {
            m.entry(i.name.to_ascii_uppercase()).or_insert_with(|| (i.name.clone(), 0)).1 += 1;
        }
    }
    m.into_values().collect()
}

fn space_label(d: &Drawing) -> String {
    if d.space == "model" { "model space".into() } else { format!("layout {}", d.space) }
}

fn log_counts(c: &Ctx) {
    let (rows, wh) = c.doc(|d| (block_counts(d), space_label(d)));
    if rows.is_empty() {
        c.log(&format!("No block references in {wh}."));
        return;
    }
    let width = rows.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(0);
    let mut out = format!("Block references in {wh}:");
    for (n, k) in &rows {
        out.push_str(&format!("\n  {n:<width$}  {k}"));
    }
    out.push_str(&format!("\n  {} in all.", plural(rows.iter().map(|r| r.1).sum(), "reference")));
    c.log(&out);
}

/// COUNT: pick an object and every object like it in the current space is selected and counted (block references by
/// block name, other objects by type and size). Pressing Enter lists the count of every block instead.
async fn count(c: Ctx, _a: Args) -> CmdResult<()> {
    let r = c.entity(ps("Select object to count or").options(&["List"]).default_str("List").allow_enter()).await?;
    let (id, list) = match r {
        EntR::Entity { id, .. } => (id, false),
        EntR::Kw(_) | EntR::Enter => (0, true),
    };
    if list {
        log_counts(&c);
        return Ok(());
    }
    let (ids, label) = c.doc(|d| {
        let Some(e) = d.by_id(id) else { return (vec![], String::new()) };
        let key = count_key(e, d);
        let ids: Vec<u64> = d.entities().iter().filter(|o| count_key(o, d) == key).map(|o| o.id).collect();
        let label = match &e.kind {
            Kind::Insert(i) => format!("Block \"{}\"", i.name),
            _ => format!("Identical {} objects", e.type_name()),
        };
        let msg = format!("{label}: {} in {}.", ids.len(), space_label(d));
        (ids, msg)
    });
    if ids.is_empty() {
        return Ok(());
    }
    c.log(&label);
    // the counted objects stay selected, so they show and can be worked on at once
    c.ed(|e| e.select(ids));
    Ok(())
}

async fn countlist(c: Ctx, _a: Args) -> CmdResult<()> {
    log_counts(&c);
    Ok(())
}

/// COUNTTABLE: a table of the block references of the current space and their counts.
async fn counttable(c: Ctx, _a: Args) -> CmdResult<()> {
    let (rows, style) = c.doc(|d| (block_counts(d), d.settings.cur_table()));
    if rows.is_empty() {
        let wh = c.doc(space_label);
        c.log_error(&format!("No block references in {wh}."));
        return Ok(());
    }
    let h = style.text_height;
    let mut cells = vec![vec!["Block Count".to_string(), String::new()], vec!["Name".to_string(), "Count".to_string()]];
    cells.extend(rows.iter().map(|(n, k)| vec![n.clone(), k.to_string()]));
    let name_w = rows.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(4).max(11) as f64 * h * 0.9 + 2.0 * h;
    let t = TableData { p: Pt::ZERO, cols: vec![name_w, 6.0 * h], rows: vec![2.0 * h; cells.len()], cells, h, merges: vec![[0, 0, 0, 1]], style: style.name.clone(), title: true, rot: 0.0, link: None };
    let shape = t.clone();
    let Some(at) = c.point(ps("Specify insertion point").preview(move |_, p| vec![pv(Entity::new(Kind::Table(TableData { p, ..shape.clone() })))])).await?.pt() else { return Ok(()) };
    c.add(Entity::new(Kind::Table(TableData { p: at, ..t })));
    c.log(&format!("Table of {} inserted.", plural(rows.len(), "block")));
    Ok(())
}

// ---------------------------------------------------------------- BURST

/// What a block reference bursts into: EXPLODE's pieces without the attribute definitions, then a text for each
/// visible attribute, showing the value the reference gave it.
fn burst_pieces(e: &Entity, d: &Drawing) -> Option<Vec<Entity>> {
    let Kind::Insert(ins) = &e.kind else { return None };
    let mut out: Vec<Entity> = explode_entity(e, d)?.into_iter().filter(|p| !matches!(&p.kind, Kind::Text(t) if t.att.is_some())).collect();
    for a in ins.attribs.iter().filter(|a| !a.hidden) {
        let mut t = Entity::new(Kind::Text(a.t.clone()));
        t.layer = e.layer.clone();
        t.color = e.color;
        out.push(t);
    }
    Some(out)
}

/// BURST: explode block references; their attributes become text with the values they show (EXPLODE would bring back
/// the block's attribute definitions instead).
async fn burst(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects").filter(|e| matches!(e.kind, Kind::Insert(_)))).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let mut made = 0;
    let mut burst = 0;
    for id in &sel.ents {
        let Some(e) = c.entity_by_id(*id) else { continue };
        let Some(parts) = c.doc(|d| burst_pieces(&e, d)) else { continue };
        let Some(idx) = c.doc(|d| d.index_of(*id)) else { continue };
        made += parts.len();
        burst += 1;
        c.doc_mut(|d| d.change(|d| d.splice(idx, 1, parts)));
    }
    c.log(&format!("{} burst into {}.", plural(burst, "block reference"), plural(made, "object")));
    c.clear_sel();
    Ok(())
}

// ---------------------------------------------------------------- NCOPY

/// The object nested in `e` (through any depth of blocks) nearest to `p`, in world coordinates; `e` itself when it is
/// not a block reference.
fn nested_at(e: &Entity, p: Pt, d: &Drawing, depth: usize) -> Option<Entity> {
    if !matches!(e.kind, Kind::Insert(_)) || depth > 16 {
        return Some(e.clone());
    }
    let parts = explode_entity(e, d)?;
    let best = parts.into_iter().filter(|q| !matches!(&q.kind, Kind::Text(t) if t.att.is_some())).map(|q| (pick_distance(&q, p, d), q)).min_by(|a, b| a.0.total_cmp(&b.0))?;
    nested_at(&best.1, p, d, depth + 1)
}

/// NCOPY: copy objects nested in block references and external references into the drawing, one pick each. The copies
/// keep their layer, colour and linetype; Enter at the base point leaves them where they are.
async fn ncopy(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut picked: Vec<Entity> = vec![];
    loop {
        let shown = picked.clone();
        c.set_preview(move |_, _| shown.iter().cloned().map(pv).collect());
        let r = c.entity(ps("Select nested objects to copy").allow_enter()).await?;
        let EntR::Entity { id, pt: p, .. } = r else { break };
        let Some(e) = c.entity_by_id(id) else { continue };
        match c.doc(|d| nested_at(&e, p, d, 0)) {
            Some(n) => {
                c.log(&format!("1 found ({})", n.type_name()));
                picked.push(Entity { id: 0, ..n });
            }
            None => c.log("Nothing to copy there."),
        }
    }
    c.clear_preview();
    if picked.is_empty() {
        return Ok(());
    }
    let shown = picked.clone();
    let base = c.point(ps("Specify base point or press Enter to copy in place").allow_enter().preview(move |_, _| shown.iter().cloned().map(pv).collect())).await?;
    let offset = match base.pt() {
        None => Pt::ZERO,
        Some(b) => {
            let shown = picked.clone();
            let to = c
                .point(ps("Specify second point").base(b).preview(move |_, p| {
                    let m = mat::translate(p.x - b.x, p.y - b.y);
                    shown
                        .iter()
                        .map(|e| {
                            let mut e = e.clone();
                            transform_entity(&mut e, &m);
                            pv(e)
                        })
                        .collect()
                }))
                .await?;
            let Some(to) = to.pt() else { return Ok(()) };
            to - b
        }
    };
    let m = mat::translate(offset.x, offset.y);
    let n = picked.len();
    c.doc_mut(|d| {
        d.change(|d| {
            for mut e in picked {
                transform_entity(&mut e, &m);
                d.add(e);
            }
        })
    });
    c.log(&format!("{} copied.", plural(n, "object")));
    Ok(())
}

// ---------------------------------------------------------------- BLOCKREPLACE

/// The defined name of block `name` (names are case-insensitive).
fn block_name(d: &Drawing, name: &str) -> Option<String> {
    d.blocks.keys().find(|k| k.eq_ignore_ascii_case(name.trim())).cloned()
}

/// A block name, picked from a reference in the drawing or typed.
async fn ask_block(c: &Ctx, msg: &str, default: Option<&str>) -> CmdResult<Option<String>> {
    loop {
        let mut spec = ps(msg).options(&["Name"]).allow_enter().filter(|e| matches!(e.kind, Kind::Insert(_)));
        if let Some(d) = default {
            spec = spec.default_str(d);
        }
        match c.entity(spec).await? {
            EntR::Entity { id, .. } => {
                if let Some(Entity { kind: Kind::Insert(i), .. }) = c.entity_by_id(id) {
                    return Ok(Some(i.name));
                }
            }
            EntR::Enter => return Ok(default.map(str::to_string)),
            EntR::Kw(_) => {
                let t = c.string(ps("Enter block name or [?]")).await?.text;
                if t.trim() == "?" {
                    let names: Vec<String> = c.doc(|d| d.blocks.keys().filter(|k| !k.starts_with('*')).cloned().collect());
                    c.log(&format!("Blocks: {}", names.join(", ")));
                    continue;
                }
                if t.trim().is_empty() {
                    continue;
                }
                match c.doc(|d| block_name(d, &t)) {
                    Some(n) => return Ok(Some(n)),
                    None => c.log_error(&format!("Block \"{}\" not found.", t.trim())),
                }
            }
        }
    }
}

/// Point every reference of block `from` (in all spaces and blocks) at block `to`, keeping its position, scale and
/// rotation; attributes are rebuilt from `to`'s definitions, keeping the values of tags both blocks have. Returns the
/// number of references changed.
pub fn replace_block(d: &mut Drawing, from: &str, to: &str) -> usize {
    let n = d.change(|d| {
        let mut n = 0;
        for e in d.all_entities_mut() {
            if let Kind::Insert(i) = &mut e.kind {
                if i.name.eq_ignore_ascii_case(from) {
                    i.name = to.to_string();
                    n += 1;
                }
            }
        }
        for b in d.blocks.values_mut() {
            for e in b.entities.iter_mut() {
                if let Kind::Insert(i) = &mut e.kind {
                    if i.name.eq_ignore_ascii_case(from) {
                        i.name = to.to_string();
                        n += 1;
                    }
                }
            }
        }
        n
    });
    crate::commands::ext_blocks::attrs::sync_block(d, to);
    n
}

/// BLOCKREPLACE: replace every reference of one block with another, then (if asked) purge the replaced block.
async fn blockreplace(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(from) = ask_block(&c, "Select a reference of the block to replace or", None).await? else { return Ok(()) };
    let Some(to) = ask_block(&c, "Select a reference of the replacement block or", None).await? else { return Ok(()) };
    if from.eq_ignore_ascii_case(&to) {
        c.log_error("The replacement must be a different block.");
        return Ok(());
    }
    let purge = c.keyword(ps("Purge the definition of the replaced block?").options(&["Yes", "No"]).default_str("Yes")).await?.kw() != Some("No");
    let n = c.doc_mut(|d| replace_block(d, &from, &to));
    let mut msg = format!("{} of \"{from}\" replaced with \"{to}\".", plural(n, "reference"));
    if purge {
        let purged = c.doc_mut(|d| {
            let used = d.all_entities().chain(d.blocks.values().flat_map(|b| b.entities.iter())).any(|e| matches!(&e.kind, Kind::Insert(i) if i.name.eq_ignore_ascii_case(&from)));
            !used && d.change(|d| d.blocks.shift_remove(&from).is_some())
        });
        if purged {
            msg.push_str(&format!(" Block \"{from}\" purged."));
        }
    }
    c.log(&msg);
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(def("Utilities", "COUNT", run(count)).title("Count").icon("blocks").desc("Counts and selects the block references (or other identical objects) like the one picked; Enter lists every block."));
    r.define(def("Utilities", "COUNTLIST", run(countlist)).title("Count List").icon("list").desc("Lists the block references of the current space with how many there are of each."));
    r.define(def("Utilities", "COUNTTABLE", run(counttable)).title("Count Table").icon("blocks").desc("Inserts a table of the blocks in the current space and their counts."));
    r.define(def("Modify", "BURST", run(burst)).title("Explode Attributes to Text").icon("explode").desc("Explodes block references, turning their attributes into text that keeps the values shown."));
    r.define(def("Modify", "NCOPY", run(ncopy)).title("Copy Nested Objects").icon("copy").desc("Copies objects nested in block references and external references into the drawing."));
    r.define(def("Insert", "BLOCKREPLACE", run(blockreplace)).title("Replace Block").icon("insert").desc("Replaces every reference of one block with another block, optionally purging the replaced one."));
}
