//! External references: XREF (attach, overlay, list, path, unload, reload, bind, detach), XATTACH and XBIND.
//!
//! A reference attaches an *open* drawing: its model space becomes a block definition marked `xref`, and the layers
//! and nested blocks it uses come along as dependent symbols named `NAME|symbol`. Binding turns the reference into
//! an ordinary block and renames the dependents to `NAME$0$symbol`.
use super::{db, ps, ps_def};
use crate::commands::layer::wildcard_match;
use crate::commands::run;
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::drawing::{Block, Drawing, XrefInfo};
use cadforge_core::entities::*;
use cadforge_core::geom::Pt;
use cadforge_core::style::Layer;
use indexmap::IndexMap;
use std::collections::BTreeSet;

/// What an xref takes from the drawing it points to.
struct Source {
    base: Pt,
    entities: Vec<Entity>,
    blocks: IndexMap<String, Block>,
    layers: Vec<Layer>,
}

impl Source {
    fn of(d: &Drawing) -> Source {
        Source {
            base: d.base,
            entities: d.model.entities.clone(),
            blocks: d.blocks.clone(),
            layers: d.layers.clone(),
        }
    }
}

/// Drawing name without directory or extension, which is how an xref block is named.
fn stem(path: &str) -> String {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let cut = [".cadforge.json", ".json", ".dxf", ".dwg"]
        .iter()
        .find_map(|e| {
            file.len()
                .checked_sub(e.len())
                .filter(|&i| file.get(i..).is_some_and(|t| t.eq_ignore_ascii_case(e)))
        });
    file[..cut.unwrap_or(file.len())].to_string()
}

/// Open drawings other than the active one (and block editors): editor id and file name or drawing name.
fn open_drawings(c: &Ctx) -> Vec<(u64, String)> {
    c.app(|a| {
        let editors = a
            .editors
            .iter()
            .enumerate()
            .filter(|(i, e)| Some(*i) != a.active && !e.doc.name.starts_with("*BlockEdit:"));
        editors
            .map(|(_, e)| {
                (
                    e.id,
                    e.doc
                        .file_name
                        .clone()
                        .unwrap_or_else(|| e.doc.name.clone()),
                )
            })
            .collect()
    })
}

fn find_open(c: &Ctx, path: &str) -> Option<(u64, String)> {
    let want = stem(path);
    open_drawings(c)
        .into_iter()
        .find(|(_, p)| stem(p).eq_ignore_ascii_case(&want))
}

fn source_of(c: &Ctx, id: u64) -> Option<Source> {
    c.app(|a| {
        a.editors
            .iter()
            .find(|e| e.id == id)
            .map(|e| Source::of(&e.doc))
    })
}

/// Blocks (transitively) referenced by the model-space entities of the source.
fn used_blocks(src: &Source) -> Vec<String> {
    let refs = |es: &[Entity]| -> Vec<String> {
        es.iter()
            .filter_map(|e| match &e.kind {
                Kind::Insert(i) => Some(i.name.clone()),
                _ => None,
            })
            .collect()
    };
    let mut out: Vec<String> = vec![];
    let mut todo = refs(&src.entities);
    while let Some(n) = todo.pop() {
        if let Some(b) = src.blocks.get(&n).filter(|_| !out.contains(&n)) {
            out.push(n);
            todo.extend(refs(&b.entities));
        }
    }
    out
}

/// Point an entity at the xref's dependent symbols, noting the layers it uses.
fn retag(e: &mut Entity, pre: &str, layers: &mut BTreeSet<String>) {
    if !e.layer.is_empty() && e.layer != "0" {
        layers.insert(e.layer.clone());
        e.layer = format!("{pre}{}", e.layer);
    }
    if let Kind::Insert(i) = &mut e.kind {
        i.name = format!("{pre}{}", i.name);
    }
}

/// (Re)build xref block `name` and its dependent layers and blocks from `src`.
fn load(d: &mut Drawing, name: &str, info: XrefInfo, src: &Source) {
    let pre = format!("{name}|");
    let mut layers = BTreeSet::new();
    let mut dependent = |es: &[Entity]| -> Vec<Entity> {
        es.iter()
            .map(|e| {
                let mut e = e.cloned_noid();
                retag(&mut e, &pre, &mut layers);
                e
            })
            .collect()
    };
    let main = dependent(&src.entities);
    let nested: Vec<(String, Pt, Vec<Entity>)> = used_blocks(src)
        .into_iter()
        .map(|n| {
            (
                format!("{pre}{n}"),
                src.blocks[&n].base,
                dependent(&src.blocks[&n].entities),
            )
        })
        .collect();
    d.change(|d| {
        for l in layers {
            if d.layer(&format!("{pre}{l}")).is_none() {
                let mut layer = src
                    .layers
                    .iter()
                    .find(|x| x.name.eq_ignore_ascii_case(&l))
                    .cloned()
                    .unwrap_or_else(|| Layer::new(&l));
                layer.name = format!("{pre}{l}");
                d.layers.push(layer);
            }
        }
        for (n, base, es) in nested {
            d.define_block(&n, base, es);
        }
        d.define_block(name, src.base, main);
        if let Some(b) = d.blocks.get_mut(name) {
            b.xref = Some(info);
        }
    });
}

/// Remove an xref, its dependent symbols and every reference to it.
fn detach(d: &mut Drawing, name: &str) {
    let pre = format!("{name}|");
    let doomed = |n: &str| n == name || n.starts_with(&pre);
    d.change(|d| {
        d.blocks.retain(|n, _| !doomed(n));
        d.layers.retain(|l| !l.name.starts_with(&pre));
        let keep = |e: &Entity| !matches!(&e.kind, Kind::Insert(i) if doomed(&i.name));
        d.model.entities.retain(&keep);
        for l in d.layouts.iter_mut() {
            l.entities.retain(&keep);
        }
        for b in d.blocks.values_mut() {
            b.entities.retain(&keep);
        }
    });
}

/// Empty an xref; the reference stays attached and can be reloaded.
fn unload(d: &mut Drawing, name: &str) {
    let pre = format!("{name}|");
    d.change(|d| {
        d.blocks.retain(|n, _| !n.starts_with(&pre));
        if let Some(b) = d.blocks.get_mut(name) {
            b.entities.clear();
            if let Some(x) = &mut b.xref {
                x.loaded = false;
            }
        }
    });
}

/// Turn an xref into an ordinary block, renaming its dependent blocks and layers to `NAME$n$symbol`.
fn bind(d: &mut Drawing, name: &str) {
    let pre = format!("{name}|");
    let fresh = |rest: &str, taken: &dyn Fn(&str) -> bool| {
        (0..)
            .map(|i| format!("{name}${i}${rest}"))
            .find(|n| !taken(n))
            .expect("endless range")
    };
    let blocks: Vec<(String, String)> = d
        .blocks
        .keys()
        .filter_map(|n| {
            n.strip_prefix(&pre)
                .map(|r| (n.clone(), fresh(r, &|x| d.blocks.contains_key(x))))
        })
        .collect();
    let layers: Vec<(String, String)> = d
        .layers
        .iter()
        .filter_map(|l| {
            l.name
                .strip_prefix(&pre)
                .map(|r| (l.name.clone(), fresh(r, &|x| d.layer(x).is_some())))
        })
        .collect();
    d.change(|d| {
        for (old, new) in &blocks {
            if let Some(mut b) = d.blocks.shift_remove(old) {
                b.name = new.clone();
                d.blocks.insert(new.clone(), b);
            }
        }
        for l in d.layers.iter_mut() {
            if let Some((_, new)) = layers.iter().find(|(o, _)| *o == l.name) {
                l.name = new.clone();
            }
        }
        for e in d.all_entities_mut() {
            if let Some((_, new)) = layers.iter().find(|(o, _)| *o == e.layer) {
                e.layer = new.clone();
            }
            if let Kind::Insert(i) = &mut e.kind {
                if let Some((_, new)) = blocks.iter().find(|(o, _)| *o == i.name) {
                    i.name = new.clone();
                }
            }
        }
        if let Some(b) = d.blocks.get_mut(name) {
            b.xref = None;
        }
    });
}

/// Names of the xref blocks matching a comma-separated list of wildcard patterns.
fn xref_names(d: &Drawing, spec: &str) -> Vec<String> {
    let pats: Vec<&str> = spec
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    d.blocks
        .values()
        .filter(|b| b.xref.is_some() && pats.iter().any(|p| wildcard_match(p, &b.name)))
        .map(|b| b.name.clone())
        .collect()
}

fn list_xrefs(c: &Ctx) {
    let rows: Vec<String> = c.doc(|d| {
        let xrefs = d
            .blocks
            .values()
            .filter_map(|b| b.xref.as_ref().map(|x| (&b.name, x)));
        xrefs
            .map(|(n, x)| {
                format!(
                    "{n:<16} {:<9} {:<8} {}",
                    if x.loaded { "Loaded" } else { "Unloaded" },
                    if x.overlay { "Overlay" } else { "Attach" },
                    x.path
                )
            })
            .collect()
    });
    c.log(&if rows.is_empty() {
        "No xrefs found.".to_string()
    } else {
        format!(
            "Xref Name        Status    Type     Path\n{}",
            rows.join("\n")
        )
    });
}

/// Ask for xref names (wildcards and commas allowed; `?` lists them); empty when none match.
async fn ask_xrefs(c: &Ctx, message: &str) -> CmdResult<Vec<String>> {
    loop {
        let t = c.string(ps(&format!("{message} or [?]"))).await?.text;
        if t == "?" {
            list_xrefs(c);
            continue;
        }
        let names = c.doc(|d| xref_names(d, &t));
        if names.is_empty() && !t.is_empty() {
            c.log_error("No matching xrefs found.");
        }
        return Ok(names);
    }
}

/// Reload an xref from its open source drawing; reports and returns false when that drawing is not open.
fn reload(c: &Ctx, name: &str) -> bool {
    let Some(info) = c.doc(|d| d.blocks.get(name).and_then(|b| b.xref.clone())) else {
        return false;
    };
    let Some(src) = find_open(c, &info.path).and_then(|(id, _)| source_of(c, id)) else {
        c.log_error(&format!(
            "Cannot reload \"{name}\": drawing \"{}\" is not open.",
            info.path
        ));
        return false;
    };
    c.doc_mut(|d| {
        load(
            d,
            name,
            XrefInfo {
                loaded: true,
                ..info
            },
            &src,
        )
    });
    true
}

/// Attach an open drawing as an xref and insert it; `overlay` is asked for unless given.
async fn attach(c: &Ctx, overlay: Option<bool>) -> CmdResult<()> {
    let open = open_drawings(c);
    let Some(first) = open.first().map(|(_, p)| stem(p)) else {
        c.log_error("No other drawing is open. Open the drawing to attach first; xrefs attach open drawings.");
        return Ok(());
    };
    let (id, path) = loop {
        let t = c
            .string(ps_def("Enter name of drawing to attach or [?]", &first))
            .await?
            .text;
        match t.as_str() {
            "" => return Ok(()),
            "?" => c.log(
                &open
                    .iter()
                    .map(|(_, p)| stem(p))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => {
                match find_open(c, &t) {
                    Some(found) => break found,
                    None => {
                        c.log_error(&format!("Drawing \"{t}\" is not open. Open it first; xrefs attach open drawings."));
                        return Ok(());
                    }
                }
            }
        }
    };
    let overlay = match overlay {
        Some(o) => o,
        None => {
            c.keyword(
                ps("Enter reference type")
                    .options(&["Attach", "Overlay"])
                    .default_str("Attach"),
            )
            .await?
            .kw()
                == Some("Overlay")
        }
    };
    let name = stem(&path);
    let existing = c.doc(|d| {
        d.blocks
            .get(&name)
            .map(|b| b.xref.as_ref().map(|x| x.loaded))
    });
    match existing {
        Some(None) => {
            c.log_error(&format!(
                "A block named \"{name}\" already exists and is not an external reference."
            ));
            return Ok(());
        }
        Some(Some(true)) => {}
        Some(Some(false)) => {
            reload(c, &name);
        }
        None => {
            let Some(src) = source_of(c, id) else {
                return Ok(());
            };
            c.doc_mut(|d| {
                load(
                    d,
                    &name,
                    XrefInfo {
                        path,
                        overlay,
                        loaded: true,
                    },
                    &src,
                )
            });
        }
    }
    c.sub_forced(
        "INSERT",
        Args::none().with("name", ArgVal::Str(name.clone())),
    )
    .await;
    // cancelling the insertion point leaves nothing attached
    let placed = c.doc(|d| {
        d.all_entities()
            .any(|e| matches!(&e.kind, Kind::Insert(i) if i.name == name))
    });
    if existing.is_none() && !placed {
        c.doc_mut(|d| detach(d, &name));
    }
    Ok(())
}

async fn xref(c: Ctx, _a: Args) -> CmdResult<()> {
    let options = [
        "?", "Bind", "Detach", "Path", "Unload", "Reload", "Overlay", "Attach",
    ];
    let k = c
        .keyword(
            ps("Enter an option")
                .options(&options)
                .default_str("Attach"),
        )
        .await?;
    match k.kw() {
        Some("?") => list_xrefs(&c),
        Some("Bind") => {
            for n in ask_xrefs(&c, "Enter xref name(s) to bind").await? {
                c.doc_mut(|d| bind(d, &n));
                c.log(&format!("Xref \"{n}\" bound."));
            }
        }
        Some("Detach") => {
            for n in ask_xrefs(&c, "Enter xref name(s) to detach").await? {
                c.doc_mut(|d| detach(d, &n));
                c.log(&format!("Xref \"{n}\" detached."));
            }
        }
        Some("Unload") => {
            for n in ask_xrefs(&c, "Enter xref name(s) to unload").await? {
                c.doc_mut(|d| unload(d, &n));
                c.log(&format!("Xref \"{n}\" unloaded."));
            }
        }
        Some("Reload") => {
            for n in ask_xrefs(&c, "Enter xref name(s) to reload").await? {
                if reload(&c, &n) {
                    c.log(&format!("Xref \"{n}\" reloaded."));
                }
            }
        }
        Some("Path") => {
            let names = ask_xrefs(&c, "Enter xref name(s) to change path").await?;
            if names.is_empty() {
                return Ok(());
            }
            let path = c.string(ps("Enter new path")).await?.text;
            if path.is_empty() {
                return Ok(());
            }
            for n in names {
                c.doc_mut(|d| {
                    d.change(|d| {
                        if let Some(x) = d.blocks.get_mut(&n).and_then(|b| b.xref.as_mut()) {
                            x.path = path.clone();
                        }
                    })
                });
                reload(&c, &n);
            }
        }
        Some("Overlay") => attach(&c, Some(true)).await?,
        _ => attach(&c, Some(false)).await?,
    }
    Ok(())
}

async fn xbind(c: Ctx, _a: Args) -> CmdResult<()> {
    for n in ask_xrefs(&c, "Enter xref name(s) to bind").await? {
        c.doc_mut(|d| bind(d, &n));
        c.log(&format!("Xref \"{n}\" bound."));
    }
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(
        db("XREF", run(xref))
            .aliases(&["XR", "-XR", "-XREF"])
            .title("External References")
            .icon("insert")
            .desc("Attaches, detaches, binds and manages external references."),
    );
    r.define(
        db(
            "XATTACH",
            run(|c, _a| async move { attach(&c, None).await }),
        )
        .aliases(&["XA"])
        .title("Attach Xref")
        .icon("insert")
        .desc("Inserts a drawing as an external reference."),
    );
    r.define(
        db("XBIND", run(xbind))
            .aliases(&["XB", "-XB", "-XBIND"])
            .title("Bind Xref")
            .icon("insert")
            .desc("Binds external references to the current drawing."),
    );
}
