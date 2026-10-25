//! CSV files: TABLEEXPORT, ATTEXT and DATAEXTRACTION write them; DATALINK and DATALINKUPDATE link a table to one.
use super::ps;
use crate::app::Event;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::Pt;
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
    cells.iter().map(|c| csv_field(c.as_ref())).collect::<Vec<_>>().join(",")
}

fn save(c: &Ctx, name: String, rows: &[String]) {
    let mut text = rows.join("\r\n");
    text.push_str("\r\n");
    let msg = format!("Wrote {} line{} to {name}.", rows.len(), if rows.len() == 1 { "" } else { "s" });
    c.app(|a| {
        a.emit(Event::SaveFile { name, data: text.into_bytes(), mime: "text/csv".into() });
        a.log(&msg, "info");
    });
}

/// TABLEEXPORT: write the cells of a table to a CSV file.
async fn tableexport(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(id) = c.pick_entity(ps("Select a table").filter(|e| matches!(e.kind, Kind::Table(_)))).await? else { return Ok(()) };
    let Some(Entity { kind: Kind::Table(t), .. }) = c.entity_by_id(id) else { return Ok(()) };
    // what the cells show: formulas give their values
    let rows: Vec<String> = cadforge_core::table_calc::display(&t).iter().map(|r| csv_row(r)).collect();
    save(&c, format!("{}-table.csv", c.name()), &rows);
    Ok(())
}

// ---------------------------------------------------------------- data links

fn is_table(e: &Entity) -> bool {
    matches!(e.kind, Kind::Table(_))
}

fn base_name(file: &str) -> &str {
    file.rsplit(['/', '\\']).next().unwrap_or(file)
}

fn same_file(a: &str, b: &str) -> bool {
    base_name(a).eq_ignore_ascii_case(base_name(b))
}

/// The CSV file: the `file` and `text` arguments, or ask the host for it (it starts the command again with them).
fn csv_or_ask(c: &Ctx, a: &Args, action: &str) -> Option<(String, String)> {
    if let Some(t) = a.str("text") {
        return Some((a.str("file").unwrap_or("data.csv").to_string(), t.to_string()));
    }
    if c.app(|app| app.ui.dialogs) {
        let args = a.clone();
        c.app(|app| app.emit(Event::UiAction { name: action.into(), args }));
    } else {
        c.log_error(&format!("{action} needs a file chooser; pass the file's text as the \"text\" argument and its name as \"file\"."));
    }
    None
}

/// DATALINK: fill a table from a CSV file and keep the link (pick a table, or press Enter for a new one).
async fn datalink(c: Ctx, a: Args) -> CmdResult<()> {
    let Some((file, text)) = csv_or_ask(&c, &a, "DATALINK") else { return Ok(()) };
    let cells = cadforge_core::table_calc::parse_csv(&text);
    if cells.is_empty() {
        c.log_error(&format!("{} has no cells.", base_name(&file)));
        return Ok(());
    }
    let (nr, nc) = (cells.len(), cells.iter().map(Vec::len).max().unwrap_or(0));
    let link = Some(DataLink { file: file.clone() });
    let target = match a.id("entity") {
        Some(id) => Some(id),
        None => c.pick_entity(ps("Select a table to link or press Enter for a new table").filter(is_table).allow_enter()).await?,
    };
    match target {
        Some(id) => {
            c.doc_mut(|d| {
                d.modify(id, |e| {
                    if let Kind::Table(t) = &mut e.kind {
                        cadforge_core::table_calc::fill_from_csv(t, &cells);
                        t.link = link.clone();
                    }
                })
            });
        }
        None => {
            let style = c.doc(|d| d.settings.cur_table());
            let h = style.text_height;
            let mut t = TableData { p: Pt::ZERO, cols: vec![8.0 * h; nc], rows: vec![2.0 * h; nr], cells: vec![], h, merges: vec![], style: style.name.clone(), title: false, rot: 0.0, link };
            cadforge_core::table_calc::fill_from_csv(&mut t, &cells);
            let shape = t.clone();
            let Some(at) = c.point(ps("Specify insertion point").preview(move |_, p| vec![PreviewItem::Entity(Entity::new(Kind::Table(TableData { p, ..shape.clone() })))])).await?.pt() else { return Ok(()) };
            t.p = at;
            c.add(Entity::new(Kind::Table(t)));
        }
    }
    c.log(&format!("Table linked to {} ({nr} rows, {nc} columns).", base_name(&file)));
    Ok(())
}

/// DATALINKUPDATE: Update reads the linked file again into every table linked to it; Write saves the tables back to
/// their files (what the cells show).
async fn datalinkupdate(c: Ctx, a: Args) -> CmdResult<()> {
    let linked: Vec<(u64, String)> = c.doc(|d| d.entities().iter().filter_map(|e| if let Kind::Table(TableData { link: Some(l), .. }) = &e.kind { Some((e.id, l.file.clone())) } else { None }).collect());
    if linked.is_empty() {
        c.log_error("No table in this drawing is linked to a data file. Use DATALINK to link one.");
        return Ok(());
    }
    let opt = match a.str("option") {
        Some(o) => o.to_string(),
        None => c.keyword(ps("Enter a data link option [Update data link/Write data link]").options(&["Update", "Write"]).default_str("Update")).await?.kw().unwrap_or("Update").to_string(),
    };
    let mut files: Vec<String> = vec![];
    for (_, f) in &linked {
        if !files.iter().any(|x| same_file(x, f)) {
            files.push(f.clone());
        }
    }
    if opt.to_ascii_uppercase().starts_with('W') {
        for f in &files {
            let csv: String = c.doc(|d| {
                d.entities().iter().find_map(|e| match &e.kind {
                    Kind::Table(t) if t.link.as_ref().is_some_and(|l| same_file(&l.file, f)) => Some(cadforge_core::table_calc::to_csv(t)),
                    _ => None,
                })
            }).unwrap_or_default();
            let name = base_name(f).to_string();
            let msg = format!("Wrote the linked table to {name}.");
            c.app(|app| {
                app.emit(Event::SaveFile { name, data: csv.into_bytes(), mime: "text/csv".into() });
                app.log(&msg, "info");
            });
        }
        return Ok(());
    }
    // Update: one file at a time (the host reads it and starts the command again with its text)
    let file = a.str("file").map(str::to_string).unwrap_or_else(|| files[0].clone());
    let ask = if a.str("text").is_some() { a.clone() } else { Args::none().with("option", ArgVal::Str("Update".into())).with("file", ArgVal::Str(file.clone())) };
    let Some((_, text)) = csv_or_ask(&c, &ask, "DATALINKUPDATE") else { return Ok(()) };
    let cells = cadforge_core::table_calc::parse_csv(&text);
    let ids: Vec<u64> = linked.iter().filter(|(_, f)| same_file(f, &file)).map(|(id, _)| *id).collect();
    if ids.is_empty() {
        c.log_error(&format!("No table is linked to {}.", base_name(&file)));
        return Ok(());
    }
    c.doc_mut(|d| {
        d.change(|d| {
            for id in &ids {
                if let Some(Entity { kind: Kind::Table(t), .. }) = d.by_id_mut(*id) {
                    cadforge_core::table_calc::fill_from_csv(t, &cells);
                }
            }
        })
    });
    c.log(&format!("Updated {} from {}.", if ids.len() == 1 { "1 linked table".to_string() } else { format!("{} linked tables", ids.len()) }, base_name(&file)));
    Ok(())
}

/// The attribute values of one block reference, by tag.
fn attrib_values(i: &InsertData) -> BTreeMap<String, String> {
    i.attribs.iter().map(|a| (a.tag.to_uppercase(), a.t.s.clone())).collect()
}

/// Block references with attributes in the selection (or all of them when nothing is selected).
async fn pick_inserts(c: &Ctx, prompt: &str) -> CmdResult<Vec<Entity>> {
    let sel = c.select(ps(prompt)).await?;
    let all: Vec<Entity> = if sel.ents.is_empty() { c.doc(|d| d.entities().clone()) } else { c.entities(&sel.ents) };
    Ok(all.into_iter().filter(|e| matches!(&e.kind, Kind::Insert(_))).collect())
}

/// ATTEXT: one CSV row per block reference with its attributes.
async fn attext(c: Ctx, _a: Args) -> CmdResult<()> {
    let inserts = pick_inserts(&c, "Select objects <all block references>").await?;
    let with_att: Vec<&Entity> = inserts.iter().filter(|e| matches!(&e.kind, Kind::Insert(i) if !i.attribs.is_empty())).collect();
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
    let mut rows = vec![csv_row(&[["BLOCK", "X", "Y", "LAYER"].map(String::from).to_vec(), tags.clone()].concat())];
    for e in with_att {
        let Kind::Insert(i) = &e.kind else { continue };
        let vals = attrib_values(i);
        let mut cells = vec![i.name.clone(), format!("{}", i.p.x), format!("{}", i.p.y), e.layer.clone()];
        cells.extend(tags.iter().map(|t| vals.get(t).cloned().unwrap_or_default()));
        rows.push(csv_row(&cells));
    }
    save(&c, format!("{}-attributes.csv", c.name()), &rows);
    Ok(())
}

/// DATAEXTRACTION: a quantity take-off of block references (with their attributes) or of all objects.
async fn dataextraction(c: Ctx, _a: Args) -> CmdResult<()> {
    let what = c.keyword(ps("Extract data from [Blocks/Objects]").options(&["Blocks", "Objects"]).default_str("Blocks")).await?;
    let ents = c.doc(|d| d.entities().clone());
    let mut rows;
    if what.kw() == Some("Objects") {
        let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
        for e in &ents {
            *counts.entry((e.type_name().to_string(), e.layer.clone())).or_default() += 1;
        }
        rows = vec![csv_row(&["Count", "Name", "Layer"])];
        rows.extend(counts.iter().map(|((n, l), k)| csv_row(&[k.to_string(), n.clone(), l.clone()])));
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
        rows = vec![csv_row(&[["Count", "Name"].map(String::from).to_vec(), tags.clone()].concat())];
        for ((name, vals), k) in &groups {
            let mut cells = vec![k.to_string(), name.clone()];
            cells.extend(tags.iter().map(|t| vals.iter().find(|(v, _)| v == t).map(|(_, x)| x.clone()).unwrap_or_default()));
            rows.push(csv_row(&cells));
        }
    }
    save(&c, format!("{}-extract.csv", c.name()), &rows);
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(def("Annotate", "TABLEEXPORT", run(tableexport)).title("Export Table").icon("table").desc("Exports data from a table object in CSV file format."));
    r.define(def("Output", "DATALINK", run(datalink)).title("Data Link").icon("table").desc("Links a table to a CSV file: the file's cells fill the table."));
    r.define(def("Output", "DATALINKUPDATE", run(datalinkupdate)).title("Update Data Links").icon("table").desc("Reads linked data files into their tables again, or writes the tables back to the files."));
    r.define(def("Blocks", "ATTEXT", run(attext)).title("Extract Attributes").icon("attedit").desc("Extracts block attribute data to a CSV file."));
    r.define(def("Output", "DATAEXTRACTION", run(dataextraction)).aliases(&["EATTEXT"]).title("Data Extraction").icon("table").desc("Extracts drawing data (block counts with attributes, or object counts) to a CSV file."));
}
