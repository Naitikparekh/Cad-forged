//! CSV extracts: TABLEEXPORT, ATTEXT and DATAEXTRACTION.
use super::ps;
use crate::app::Event;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use std::collections::BTreeMap;

/// One CSV field, quoted when it holds a delimiter, quote or line break.
pub fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn csv_row<S: AsRef<str>>(cells: &[S]) -> String {
    cells
        .iter()
        .map(|c| csv_field(c.as_ref()))
        .collect::<Vec<_>>()
        .join(",")
}

fn save(c: &Ctx, name: String, rows: &[String]) {
    let mut text = rows.join("\r\n");
    text.push_str("\r\n");
    let msg = format!(
        "Wrote {} line{} to {name}.",
        rows.len(),
        if rows.len() == 1 { "" } else { "s" }
    );
    c.app(|a| {
        a.emit(Event::SaveFile {
            name,
            data: text.into_bytes(),
            mime: "text/csv".into(),
        });
        a.log(&msg, "info");
    });
}

/// TABLEEXPORT: write the cells of a table to a CSV file.
async fn tableexport(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(id) = c
        .pick_entity(ps("Select a table").filter(|e| matches!(e.kind, Kind::Table(_))))
        .await?
    else {
        return Ok(());
    };
    let Some(Entity {
        kind: Kind::Table(t),
        ..
    }) = c.entity_by_id(id)
    else {
        return Ok(());
    };
    let rows: Vec<String> = t.cells.iter().map(|r| csv_row(r)).collect();
    save(&c, format!("{}-table.csv", c.name()), &rows);
    Ok(())
}

/// The attribute values of one block reference, by tag.
fn attrib_values(i: &InsertData) -> BTreeMap<String, String> {
    i.attribs
        .iter()
        .map(|a| (a.tag.to_uppercase(), a.t.s.clone()))
        .collect()
}

/// Block references with attributes in the selection (or all of them when nothing is selected).
async fn pick_inserts(c: &Ctx, prompt: &str) -> CmdResult<Vec<Entity>> {
    let sel = c.select(ps(prompt)).await?;
    let all: Vec<Entity> = if sel.ents.is_empty() {
        c.doc(|d| d.entities().clone())
    } else {
        c.entities(&sel.ents)
    };
    Ok(all
        .into_iter()
        .filter(|e| matches!(&e.kind, Kind::Insert(_)))
        .collect())
}

/// ATTEXT: one CSV row per block reference with its attributes.
async fn attext(c: Ctx, _a: Args) -> CmdResult<()> {
    let inserts = pick_inserts(&c, "Select objects <all block references>").await?;
    let with_att: Vec<&Entity> = inserts
        .iter()
        .filter(|e| matches!(&e.kind, Kind::Insert(i) if !i.attribs.is_empty()))
        .collect();
    if with_att.is_empty() {
        c.log_error("No block references with attributes were found.");
        return Ok(());
    }
    let mut tags: Vec<String> = vec![];
    for e in &with_att {
        if let Kind::Insert(i) = &e.kind {
            for a in &i.attribs {
                let t = a.tag.to_uppercase();
                if !tags.contains(&t) {
                    tags.push(t);
                }
            }
        }
    }
    let mut rows = vec![csv_row(
        &[
            ["BLOCK", "X", "Y", "LAYER"].map(String::from).to_vec(),
            tags.clone(),
        ]
        .concat(),
    )];
    for e in with_att {
        let Kind::Insert(i) = &e.kind else { continue };
        let vals = attrib_values(i);
        let mut cells = vec![
            i.name.clone(),
            format!("{}", i.p.x),
            format!("{}", i.p.y),
            e.layer.clone(),
        ];
        cells.extend(
            tags.iter()
                .map(|t| vals.get(t).cloned().unwrap_or_default()),
        );
        rows.push(csv_row(&cells));
    }
    save(&c, format!("{}-attributes.csv", c.name()), &rows);
    Ok(())
}

/// DATAEXTRACTION: a quantity take-off of block references (with their attributes) or of all objects.
async fn dataextraction(c: Ctx, _a: Args) -> CmdResult<()> {
    let what = c
        .keyword(
            ps("Extract data from [Blocks/Objects]")
                .options(&["Blocks", "Objects"])
                .default_str("Blocks"),
        )
        .await?;
    let ents = c.doc(|d| d.entities().clone());
    let mut rows;
    if what.kw() == Some("Objects") {
        let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
        for e in &ents {
            *counts
                .entry((e.type_name().to_string(), e.layer.clone()))
                .or_default() += 1;
        }
        rows = vec![csv_row(&["Count", "Name", "Layer"])];
        rows.extend(
            counts
                .iter()
                .map(|((n, l), k)| csv_row(&[k.to_string(), n.clone(), l.clone()])),
        );
    } else {
        let mut tags: Vec<String> = vec![];
        let mut groups: BTreeMap<(String, Vec<(String, String)>), usize> = BTreeMap::new();
        for e in &ents {
            let Kind::Insert(i) = &e.kind else { continue };
            let vals: Vec<(String, String)> = attrib_values(i).into_iter().collect();
            for (t, _) in &vals {
                if !tags.contains(t) {
                    tags.push(t.clone());
                }
            }
            *groups.entry((i.name.clone(), vals)).or_default() += 1;
        }
        if groups.is_empty() {
            c.log_error("There are no block references in the drawing.");
            return Ok(());
        }
        tags.sort();
        rows = vec![csv_row(
            &[["Count", "Name"].map(String::from).to_vec(), tags.clone()].concat(),
        )];
        for ((name, vals), k) in &groups {
            let mut cells = vec![k.to_string(), name.clone()];
            cells.extend(tags.iter().map(|t| {
                vals.iter()
                    .find(|(v, _)| v == t)
                    .map(|(_, x)| x.clone())
                    .unwrap_or_default()
            }));
            rows.push(csv_row(&cells));
        }
    }
    save(&c, format!("{}-extract.csv", c.name()), &rows);
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(
        def("Annotate", "TABLEEXPORT", run(tableexport))
            .title("Export Table")
            .icon("table")
            .desc("Exports data from a table object in CSV file format."),
    );
    r.define(
        def("Blocks", "ATTEXT", run(attext))
            .title("Extract Attributes")
            .icon("attedit")
            .desc("Extracts block attribute data to a CSV file."),
    );
    r.define(def("Output", "DATAEXTRACTION", run(dataextraction)).aliases(&["EATTEXT"]).title("Data Extraction").icon("table").desc("Extracts drawing data (block counts with attributes, or object counts) to a CSV file."));
}
