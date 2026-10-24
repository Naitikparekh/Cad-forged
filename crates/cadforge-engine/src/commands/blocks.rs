//! Blocks, groups, purge and rename (port of `src/commands/blocks.js`).
use super::helpers::*;
use super::{def, run};
use crate::app::Event;
use crate::ctx::*;
use crate::registry::{CommandDef, Registry};
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use std::collections::HashSet;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn db(name: &str, f: crate::registry::RunFn) -> CommandDef {
    def("Block", name, f)
}

fn valid_name(n: &str) -> bool {
    !n.is_empty() && !n.chars().any(|c| "<>/\\\":;?*|,=`".contains(c)) && n.len() <= 255
}

async fn block(c: Ctx, args: Args) -> CmdResult<()> {
    let mut name = args.str("name").map(str::to_string);
    if name.is_none() {
        name = Some(c.string(ps("Enter block name or [?]")).await?.text);
    }
    let name = name.unwrap_or_default();
    if name.is_empty() {
        return Ok(());
    }
    if name == "?" {
        let names: Vec<String> = c.doc(|d| d.blocks.keys().cloned().collect());
        c.log(&if names.is_empty() { "No blocks defined.".to_string() } else { names.join("\n") });
        return Ok(());
    }
    if !valid_name(&name) {
        c.log_error("Invalid block name.");
        return Ok(());
    }
    if c.doc(|d| d.blocks.contains_key(&name)) {
        let y = c.keyword(ps(&format!("Block \"{name}\" already exists. Redefine it? [Yes/No]")).options(&["Yes", "No"]).default_str("No")).await?;
        if y.kw() != Some("Yes") {
            return Ok(());
        }
    }
    let base = match args.pt("base") {
        Some(b) => b,
        None => match c.point(ps("Specify insertion base point")).await?.pt() {
            Some(b) => b,
            None => return Ok(()),
        },
    };
    let ids: Vec<u64> = match args.ids("sel") {
        Some(s) => s.to_vec(),
        None => c.select(ps("Select objects")).await?.ents,
    };
    if ids.is_empty() {
        return Ok(());
    }
    let mode = args.str("retain").unwrap_or("convert").to_string();
    let inner: Vec<Entity> = c
        .entities(&ids)
        .iter()
        .map(|e| {
            let mut k = e.cloned_noid();
            k.grp = None;
            k
        })
        .collect();
    let n = inner.len();
    c.doc_mut(|d| {
        d.define_block(&name, base, inner);
        d.will_change();
        if mode != "retain" {
            d.remove_all(&ids);
            if mode == "convert" {
                d.add(Entity::insert(&name, base));
            }
        }
    });
    c.clear_sel();
    c.log(&format!("Block \"{name}\" defined with {n} object{}.", if n == 1 { "" } else { "s" }));
    Ok(())
}

async fn insert(c: Ctx, args: Args) -> CmdResult<()> {
    let (mem_scale, mem_rot) = c.app(|a| (a.mem.insert_scale, a.mem.insert_rot));
    let names: Vec<String> = c.doc(|d| d.blocks.keys().cloned().collect());
    let mut name = args.str("name").map(str::to_string);
    if name.is_none() {
        match c.dialog(DialogReq::Insert { names: names.clone() }).await? {
            Some(DialogReply::Text(t)) => name = Some(t),
            Some(_) => return Ok(()),
            None => name = Some(c.string(match names.first() { Some(n) => ps("Enter block name or [?]").default_str(n), None => ps("Enter block name or [?]") }).await?.text),
        }
    }
    let name = name.unwrap_or_default();
    if name.is_empty() {
        return Ok(());
    }
    if name == "?" {
        c.log(&if names.is_empty() { "No blocks defined.".to_string() } else { names.join("\n") });
        return Ok(());
    }
    let Some(key) = names.iter().find(|n| n.to_lowercase() == name.to_lowercase()).cloned() else {
        c.log_error(&format!("Block \"{name}\" not found."));
        return Ok(());
    };
    let (mut sx, mut sy, mut rot) = (args.num("scale").unwrap_or(mem_scale), args.num("scale").unwrap_or(mem_scale), args.num("rotation").unwrap_or(mem_rot));
    loop {
        let (k2, s1, s2, r1) = (key.clone(), sx, sy, rot);
        let make = move |p: Pt| -> Entity {
            let mut e = Entity::insert(&k2, p);
            if let Kind::Insert(i) = &mut e.kind {
                i.sx = s1;
                i.sy = s2;
                i.rot = r1;
            }
            e
        };
        let m2 = make.clone();
        let r = c.point(ps("Specify insertion point").options(&["Basepoint", "Scale", "X", "Y", "Rotate"]).preview(move |_, p| vec![pv(m2(p))])).await?;
        match r.kw() {
            Some("Scale") => {
                let s = c.number(ps("Specify scale factor for XYZ axes").default_num(sx).nonzero()).await?;
                if let Some(v) = s.value().filter(|v| *v != 0.0) {
                    sx = v;
                    sy = v;
                }
                continue;
            }
            Some("X") => {
                let s = c.number(ps("Specify X scale factor").default_num(sx).nonzero()).await?;
                if let Some(v) = s.value().filter(|v| *v != 0.0) {
                    sx = v;
                }
                continue;
            }
            Some("Y") => {
                let s = c.number(ps("Specify Y scale factor").default_num(sy).nonzero()).await?;
                if let Some(v) = s.value().filter(|v| *v != 0.0) {
                    sy = v;
                }
                continue;
            }
            Some("Rotate") => {
                let s = c.angle(ps("Specify rotation angle").default_num(rot)).await?;
                if let Some(v) = s.value() {
                    rot = v;
                }
                continue;
            }
            Some("Basepoint") => {
                c.log("Base point is fixed by the block definition.");
                continue;
            }
            _ => {}
        }
        let Some(p) = r.pt() else { return Ok(()) };
        c.app(|a| {
            a.mem.insert_scale = sx;
            a.mem.insert_rot = rot;
        });
        c.add(make(p));
        if !args.flag("multiple") {
            return Ok(());
        }
    }
}

async fn wblock(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects")).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let Some(base) = c.point(ps("Specify insertion base point")).await?.pt() else { return Ok(()) };
    let shifted = transformed_clones(&c.entities(&sel.ents), &mat::translate(-base.x, -base.y));
    let typed = c.string(ps("Enter file name").default_str("Block")).await?.text;
    let name = typed.trim().trim_end_matches(".dxf").trim_end_matches(".DXF").to_string();
    if name.is_empty() {
        return Ok(());
    }
    // the new drawing carries the layers, settings and every block definition the objects insert
    let out = c.doc(|d| {
        let mut out = cadforge_core::drawing::Drawing::new(&name);
        out.layers = d.layers.clone();
        out.settings = d.settings.clone();
        let mut todo: Vec<String> = vec![];
        let want = |es: &[Entity], todo: &mut Vec<String>| {
            for e in es {
                if let Kind::Insert(i) = &e.kind {
                    if !todo.contains(&i.name) {
                        todo.push(i.name.clone());
                    }
                }
            }
        };
        want(&shifted, &mut todo);
        let mut i = 0;
        while i < todo.len() {
            if let Some(b) = d.blocks.get(&todo[i]) {
                want(&b.entities, &mut todo);
                out.define_block(&b.name, b.base, b.entities.clone());
            }
            i += 1;
        }
        for mut e in shifted.clone() {
            e.id = 0;
            out.add(e);
        }
        // pictures used by the objects or by the blocks they insert
        let pics: Vec<String> = out.all_entities().chain(out.blocks.values().flat_map(|b| b.entities.iter())).filter_map(|e| if let Kind::Image(i) = &e.kind { Some(i.name.clone()) } else { None }).collect();
        for n in pics {
            if let Some(def) = d.images.get(&n) {
                out.images.insert(n, def.clone());
            }
        }
        out
    });
    let text = cadforge_core::io::write_dxf(&out);
    c.app(|a| {
        a.emit(Event::SaveFile { name: format!("{name}.dxf"), data: text.into_bytes(), mime: "application/dxf".into() });
        a.log(&format!("Wrote {} object{} to {name}.dxf.", shifted.len(), if shifted.len() == 1 { "" } else { "s" }), "info");
    });
    Ok(())
}

async fn group(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects or [Name/Description]")).await?;
    if sel.ents.len() < 2 {
        c.log_error("Select at least two objects to group.");
        return Ok(());
    }
    let serial = c.app(|a| {
        a.mem.group_serial += 1;
        a.mem.group_serial
    });
    let id = format!("*A{serial}_{}", c.doc(|d| d.next_id));
    c.doc_mut(|d| {
        d.change(|d| {
            for i in &sel.ents {
                if let Some(e) = d.by_id_mut(*i) {
                    e.grp = Some(id.clone());
                }
            }
        })
    });
    c.log(&format!("Group *A{serial} has been created."));
    Ok(())
}

async fn ungroup(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select a group")).await?;
    c.doc_mut(|d| {
        d.change(|d| {
            for i in &sel.ents {
                if let Some(e) = d.by_id_mut(*i) {
                    e.grp = None;
                }
            }
        })
    });
    Ok(())
}

async fn purge(c: Ctx, _a: Args) -> CmdResult<()> {
    let (unused_blocks, unused_layers): (Vec<String>, Vec<String>) = c.doc(|d| {
        let mut used: HashSet<String> = HashSet::new();
        fn walk(list: &[Entity], d: &cadforge_core::drawing::Drawing, used: &mut HashSet<String>) {
            for e in list {
                if let Kind::Insert(i) = &e.kind {
                    if used.insert(i.name.clone()) {
                        if let Some(b) = d.blocks.get(&i.name) {
                            walk(&b.entities, d, used);
                        }
                    }
                }
            }
        }
        walk(&d.model.entities, d, &mut used);
        for l in &d.layouts {
            walk(&l.entities, d, &mut used);
        }
        let blocks = d.blocks.keys().filter(|n| !used.contains(*n)).cloned().collect();
        let layers = d.layers.iter().filter(|l| l.name != "0" && l.name != d.cur.layer && !d.layer_usage(&l.name)).map(|l| l.name.clone()).collect();
        (blocks, layers)
    });
    if unused_blocks.is_empty() && unused_layers.is_empty() {
        c.log("No unreferenced named objects found to purge.");
        return Ok(());
    }
    c.doc_mut(|d| {
        d.change(|d| {
            for n in &unused_blocks {
                d.blocks.shift_remove(n);
            }
            d.layers.retain(|l| !unused_layers.contains(&l.name));
        })
    });
    for n in &unused_blocks {
        c.log(&format!("Deleting block \"{n}\"."));
    }
    for n in &unused_layers {
        c.log(&format!("Deleting layer \"{n}\"."));
    }
    c.log(&format!("{} items purged.", unused_blocks.len() + unused_layers.len()));
    Ok(())
}

async fn rename(c: Ctx, _a: Args) -> CmdResult<()> {
    let k = c.keyword(ps("Enter object type to rename [Block/LAyer/LType/Style]").options(&["Block", "LAyer"]).default_str("LAyer")).await?;
    let is_block = k.kw() == Some("Block");
    let what = if is_block { "block" } else { "layer" };
    let old = c.string(ps(&format!("Enter old {what} name"))).await?.text;
    if old.is_empty() {
        return Ok(());
    }
    let new = c.string(ps(&format!("Enter new {what} name"))).await?.text;
    if new.is_empty() {
        return Ok(());
    }
    if is_block {
        if !c.doc(|d| d.blocks.contains_key(&old)) {
            return Err("Block not found.".into());
        }
        if c.doc(|d| d.blocks.contains_key(&new)) {
            return Err("That name is already in use.".into());
        }
        c.doc_mut(|d| {
            d.change(|d| {
                if let Some(mut b) = d.blocks.shift_remove(&old) {
                    b.name = new.clone();
                    d.blocks.insert(new.clone(), b);
                }
                for e in d.all_entities_mut() {
                    if let Kind::Insert(i) = &mut e.kind {
                        if i.name == old {
                            i.name = new.clone();
                        }
                    }
                }
            })
        });
    } else {
        c.doc_mut(|d| d.rename_layer(&old, &new))?;
    }
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(db("BLOCK", run(block)).aliases(&["B"]).title("Create Block").icon("block").desc("Creates a block definition from selected objects."));
    r.define(db("INSERT", run(insert)).aliases(&["I", "-INSERT"]).title("Insert Block").icon("insert").desc("Inserts a block or drawing into the current drawing."));
    r.define(db("WBLOCK", run(wblock)).aliases(&["W"]).title("Write Block").icon("wblock").desc("Writes objects or a block to a new drawing file."));
    r.define(db("GROUP", run(group)).aliases(&["G"]).title("Group").icon("group").desc("Creates and manages saved sets of objects called groups."));
    r.define(db("UNGROUP", run(ungroup)).aliases(&["UG"]).title("Ungroup").icon("ungroup").desc("Dissolves a group."));
    r.define(db("PURGE", run(purge)).aliases(&["PU"]).title("Purge").icon("purge").desc("Removes unused items, such as block definitions and layers, from the drawing."));
    r.define(db("-RENAME", run(rename)).aliases(&["REN", "RENAME"]).title("Rename").icon("rename").desc("Changes the names assigned to items such as layers and blocks."));
}
