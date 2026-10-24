//! Selection commands: SELECT, FILTER, MULTIPLE, GROUPEDIT, GROUPS.
use super::{dm, pref_text, ps, set_pref};
use crate::commands::layer::wildcard_match;
use crate::commands::run;
use crate::ctx::*;
use crate::registry::{CommandDef, Registry};
use crate::runtime::*;
use crate::sysvars::{parse_color, Value};
use cadforge_core::entities::*;
use cadforge_core::style::ltype_key;
use std::collections::BTreeMap;

fn dsel(name: &str, f: crate::registry::RunFn) -> CommandDef {
    crate::commands::def("System", name, f).no_pickfirst()
}

// ---------------------------------------------------------------- SELECT

/// Builds a selection set. The objects stay selected (pick-first) for the next command and are also the
/// "Previous" set, so `P` at any later "Select objects" prompt selects them again.
async fn select(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects").solids()).await?;
    if sel.is_empty() {
        return Ok(());
    }
    c.ed(|e| e.sel = sel.ents.iter().chain(&sel.solids).copied().collect());
    c.emit();
    Ok(())
}

// ---------------------------------------------------------------- FILTER

const KINDS: &[&str] = &[
    "Object", "Layer", "Color", "LType", "LWeight", "Name", "Text", "Radius",
];
const FILTER_KEY: &str = "_FILTERLIST";

/// One entry of the filter list: `kind op value`.
#[derive(Clone, Debug, PartialEq)]
struct Crit {
    kind: String,
    op: String,
    value: String,
}

fn load_list(c: &Ctx) -> Vec<Crit> {
    pref_text(c, FILTER_KEY, "")
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some(Crit {
                kind: f.next()?.into(),
                op: f.next()?.into(),
                value: f.next()?.into(),
            })
        })
        .collect()
}

fn store_list(c: &Ctx, list: &[Crit]) {
    let text: Vec<String> = list
        .iter()
        .map(|k| {
            format!(
                "{}\t{}\t{}",
                k.kind,
                k.op,
                k.value.replace(['\t', '\n'], " ")
            )
        })
        .collect();
    set_pref(c, FILTER_KEY, Value::Str(text.join("\n")));
}

fn show(k: &Crit) -> String {
    format!("{} {} {}", k.kind, k.op, k.value)
}

/// Canonical object type of a name typed by the user (`Polyline`, `mtext`, `Block Reference`...).
fn object_type(name: &str) -> Option<&'static str> {
    Some(match name.to_lowercase().replace(' ', "").as_str() {
        "line" => "line",
        "circle" => "circle",
        "arc" => "arc",
        "polyline" | "lwpolyline" | "pline" => "pline",
        "ellipse" => "ellipse",
        "spline" => "spline",
        "point" => "point",
        "text" | "dtext" => "text",
        "mtext" => "mtext",
        "dimension" | "dim" => "dim",
        "leader" | "mleader" => "leader",
        "hatch" => "hatch",
        "insert" | "block" | "blockreference" => "insert",
        "xline" => "xline",
        "ray" => "ray",
        "viewport" => "viewport",
        "table" => "table",
        "wipeout" => "wipeout",
        "region" => "region",
        _ => return None,
    })
}

fn type_of(e: &Entity) -> &'static str {
    match &e.kind {
        Kind::Text(t) if t.multi => "mtext",
        _ => e.type_name(),
    }
}

/// Does the entity satisfy one criterion (including its operator)?
fn holds(k: &Crit, e: &Entity) -> bool {
    let cmp = |have: f64, want: f64| match k.op.as_str() {
        "=" => (have - want).abs() < 1e-9,
        "!=" => (have - want).abs() >= 1e-9,
        "<" => have < want,
        ">" => have > want,
        "<=" => have <= want + 1e-9,
        _ => have >= want - 1e-9,
    };
    let eq = match k.kind.as_str() {
        "Object" => type_of(e) == k.value,
        "Layer" => wildcard_match(&k.value, if e.layer.is_empty() { "0" } else { &e.layer }),
        "Color" => {
            parse_color(&k.value).ok()
                == Some(
                    e.color
                        .filter(|c| *c != cadforge_core::style::Color::ByLayer),
                )
        }
        "LType" => {
            let own = e
                .ltype
                .as_deref()
                .filter(|l| !l.eq_ignore_ascii_case("BYLAYER"))
                .unwrap_or("BYLAYER");
            own.eq_ignore_ascii_case(ltype_key(&k.value).unwrap_or(&k.value))
        }
        "Name" => matches!(&e.kind, Kind::Insert(i) if wildcard_match(&k.value, &i.name)),
        "Text" => match &e.kind {
            Kind::Text(t) => wildcard_match(&k.value, &t.s),
            Kind::Leader { s, .. } => wildcard_match(&k.value, s),
            _ => false,
        },
        "LWeight" => {
            return matches!(e.lw, Some(w) if w >= 0)
                && cmp(
                    e.lw.unwrap_or(0) as f64,
                    k.value.parse::<f64>().unwrap_or(f64::NAN) * 100.0,
                )
        }
        _ => {
            let r = match &e.kind {
                Kind::Circle { r, .. } | Kind::Arc { r, .. } => *r,
                _ => return false,
            };
            return cmp(r, k.value.parse().unwrap_or(f64::NAN));
        }
    };
    match k.op.as_str() {
        "!=" => !eq,
        _ => eq,
    }
}

/// Entries of one kind: any `=` may hold, every other operator must hold. Different kinds all have to pass.
fn passes(list: &[Crit], e: &Entity) -> bool {
    KINDS.iter().all(|kind| {
        let group: Vec<&Crit> = list.iter().filter(|k| k.kind == *kind).collect();
        let (eqs, rest): (Vec<&Crit>, Vec<&Crit>) = group.into_iter().partition(|k| k.op == "=");
        (eqs.is_empty() || eqs.iter().any(|k| holds(k, e))) && rest.iter().all(|k| holds(k, e))
    })
}

/// Validate the typed value of a new entry, returning the text to store.
fn check_value(kind: &str, value: &str) -> Result<String, String> {
    let v = value.trim();
    match kind {
        "Object" => object_type(v)
            .map(str::to_string)
            .ok_or_else(|| format!("Unknown object type \"{v}\".")),
        "Color" => parse_color(v).map(|_| v.to_string()),
        "LType" => {
            if v.eq_ignore_ascii_case("BYLAYER") || v.eq_ignore_ascii_case("BYBLOCK") {
                Ok(v.to_uppercase())
            } else {
                ltype_key(v)
                    .map(str::to_string)
                    .ok_or_else(|| format!("Linetype \"{v}\" not found."))
            }
        }
        "LWeight" | "Radius" => v
            .parse::<f64>()
            .ok()
            .filter(|x| x.is_finite())
            .map(|_| v.to_string())
            .ok_or_else(|| "Requires a numeric value.".into()),
        _ if v.is_empty() => Err("A value is required.".into()),
        _ => Ok(v.to_string()),
    }
}

async fn filter(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut list = load_list(&c);
    loop {
        c.log(&format!(
            "Filter list: {}",
            if list.is_empty() {
                "(empty)".to_string()
            } else {
                list.iter().map(show).collect::<Vec<_>>().join("; ")
            }
        ));
        let r = c
            .keyword(
                ps("Enter filter option")
                    .options(&[
                        "ADd to list",
                        "Delete",
                        "Clear list",
                        "List",
                        "Apply",
                        "eXit",
                    ])
                    .default_str("Apply"),
            )
            .await?;
        match r.kw() {
            Some("ADd to list") => {
                let Some(kind) = c
                    .keyword(ps("Enter filter type").options(KINDS))
                    .await?
                    .kw()
                    .map(str::to_string)
                else {
                    continue;
                };
                let numeric = matches!(kind.as_str(), "LWeight" | "Radius");
                let ops: &[&str] = if numeric {
                    &["=", "!=", "<", ">", "<=", ">="]
                } else {
                    &["=", "!="]
                };
                let Some(op) = c
                    .keyword(ps("Enter operator").options(ops).default_str("="))
                    .await?
                    .kw()
                    .map(str::to_string)
                else {
                    continue;
                };
                let what = match kind.as_str() {
                    "Object" => "Enter object type (Line, Circle, Arc, Polyline, Text, MText, Insert, Hatch...)",
                    "Layer" => "Enter layer name (wildcards allowed)",
                    "Color" => "Enter color (index, name, BYLAYER or BYBLOCK)",
                    "LType" => "Enter linetype name",
                    "LWeight" => "Enter lineweight in mm",
                    "Name" => "Enter block name (wildcards allowed)",
                    "Text" => "Enter text string (wildcards allowed)",
                    _ => "Enter radius",
                };
                let v = c.string(ps(what)).await?;
                match check_value(&kind, &v.text) {
                    Ok(value) => {
                        let k = Crit { kind, op, value };
                        c.log(&format!("Added: {}", show(&k)));
                        list.push(k);
                        store_list(&c, &list);
                    }
                    Err(m) => c.log_error(&m),
                }
            }
            Some("Delete") => {
                if list.is_empty() {
                    c.log_error("The filter list is empty.");
                    continue;
                }
                let n = c
                    .number(
                        ps(&format!(
                            "Enter the number of the entry to delete (1-{})",
                            list.len()
                        ))
                        .integer()
                        .min(1.0)
                        .max(list.len() as f64),
                    )
                    .await?;
                if let Some(i) = n.value() {
                    let gone = list.remove(i as usize - 1);
                    c.log(&format!("Deleted: {}", show(&gone)));
                    store_list(&c, &list);
                }
            }
            Some("Clear list") => {
                list.clear();
                store_list(&c, &list);
                c.log("Filter list cleared.");
            }
            Some("List") => {
                for (i, k) in list.iter().enumerate() {
                    c.log(&format!("{}. {}", i + 1, show(k)));
                }
            }
            Some("Apply") => {
                if list.is_empty() {
                    c.log_error("Add at least one entry to the filter list first.");
                    continue;
                }
                let l2 = list.clone();
                let sel = c
                    .select(ps("Select objects").filter(move |e| passes(&l2, e)))
                    .await?;
                if !sel.is_empty() {
                    c.ed(|e| e.sel = sel.ents.iter().copied().collect());
                    c.emit();
                }
                return Ok(());
            }
            _ => return Ok(()),
        }
    }
}

// ---------------------------------------------------------------- MULTIPLE

/// Repeats a command until it is cancelled. Everything done counts as one undo step.
async fn multiple(c: Ctx, _a: Args) -> CmdResult<()> {
    let name = c.string(ps("Enter command name to repeat")).await?.text;
    if name.is_empty() {
        return Ok(());
    }
    let Some(def) = c.app(|a| a.registry.find(&name)) else {
        c.log_error(&format!("Unknown command \"{name}\".  Press F1 for help."));
        return Ok(());
    };
    if def.name == "MULTIPLE" {
        c.log_error("MULTIPLE cannot repeat itself.");
        return Ok(());
    }
    loop {
        let mut fut = (def.run)(c.clone(), Args::none());
        let mut prompted = false;
        std::future::poll_fn(|cx| {
            let r = fut.as_mut().poll(cx);
            prompted |= r.is_pending();
            r
        })
        .await?;
        if !prompted {
            c.log_error(&format!(
                "{} does not wait for input, so it is not repeated.",
                def.name
            ));
            return Ok(());
        }
    }
}

// ---------------------------------------------------------------- GROUPEDIT / GROUPS

/// The name AutoCAD shows for an anonymous group (`*A3`).
fn group_name(id: &str) -> &str {
    id.split('_').next().unwrap_or(id)
}

async fn groupedit(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut gid: Option<String> = None;
    while gid.is_none() {
        let r = c.entity(ps("Select group or").options(&["Name"])).await?;
        if r.kw() == Some("Name") {
            let n = c.string(ps("Enter group name")).await?.text;
            let want = n.trim().trim_start_matches('*').to_uppercase();
            gid = c.doc(|d| {
                d.entities()
                    .iter()
                    .filter_map(|e| e.grp.clone())
                    .find(|g| group_name(g).trim_start_matches('*').to_uppercase() == want)
            });
            if gid.is_none() {
                c.log_error(&format!("Group \"{}\" not found.", n.trim()));
            }
        } else if let Some(id) = r.id() {
            gid = c.entity_by_id(id).and_then(|e| e.grp);
            if gid.is_none() {
                c.log_error("The object is not in a group.");
            }
        } else {
            return Ok(());
        }
    }
    let gid = gid.expect("a group was chosen");
    loop {
        let r = c
            .keyword(ps("Enter a group option").options(&["Add objects", "Remove objects"]))
            .await?;
        match r.kw() {
            Some("Add objects") => {
                let sel = c
                    .select(ps("Select objects to add to group").pickfirst(false))
                    .await?;
                let n = sel.ents.len();
                c.doc_mut(|d| {
                    d.change(|d| {
                        for id in &sel.ents {
                            if let Some(e) = d.by_id_mut(*id) {
                                e.grp = Some(gid.clone());
                            }
                        }
                    })
                });
                c.log(&format!(
                    "{n} object(s) added to group {}.",
                    group_name(&gid)
                ));
            }
            Some("Remove objects") => loop {
                let g = gid.clone();
                let r = c
                    .entity(
                        ps("Select objects to remove from group")
                            .allow_enter()
                            .filter(move |e| e.grp.as_deref() == Some(g.as_str())),
                    )
                    .await?;
                let Some(id) = r.id() else { break };
                c.doc_mut(|d| {
                    d.change(|d| {
                        if let Some(e) = d.by_id_mut(id) {
                            e.grp = None;
                        }
                    })
                });
                c.log(&format!(
                    "1 object removed from group {}.",
                    group_name(&gid)
                ));
            },
            _ => return Ok(()),
        }
    }
}

/// Lists the groups of the drawing with their members.
async fn groups(c: Ctx, _a: Args) -> CmdResult<()> {
    let found: BTreeMap<String, Vec<&'static str>> = c.doc(|d| {
        let mut m: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
        for e in d.entities() {
            if let Some(g) = &e.grp {
                m.entry(g.clone()).or_default().push(e.type_name());
            }
        }
        m
    });
    if found.is_empty() {
        c.log("No groups are defined.");
    }
    for (g, members) in &found {
        c.log(&format!(
            "{}: {} object(s) - {}",
            group_name(g),
            members.len(),
            members.join(", ")
        ));
    }
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(dsel("SELECT", run(select)).title("Select").icon("cursor").desc("Places selected objects in the Previous selection set and keeps them selected for the next command."));
    r.define(dsel("FILTER", run(filter)).aliases(&["FI"]).title("Filter").icon("qselect").desc("Creates a list of requirements that an object must meet to be included in a selection set (command-line form of the Object Selection Filters dialog)."));
    r.define(
        dm("MULTIPLE", run(multiple))
            .title("Multiple")
            .desc("Repeats the next command until you cancel it.")
            .no_pickfirst(),
    );
    r.define(
        crate::commands::def("Blocks", "GROUPEDIT", run(groupedit))
            .title("Edit Group")
            .icon("group")
            .desc("Adds objects to and removes objects from a group.")
            .no_pickfirst(),
    );
    r.define(
        crate::commands::def("Blocks", "GROUPS", run(groups))
            .title("Groups")
            .icon("group")
            .desc("Lists the groups defined in the drawing.")
            .no_pickfirst(),
    );
}
