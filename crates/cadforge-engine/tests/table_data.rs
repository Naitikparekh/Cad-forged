//! Table formulas (cells starting with `=` show their values) and data links to CSV files (DATALINK,
//! DATALINKUPDATE), kept through DXF and DWG.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::app::Event;
use cadforge_engine::runtime::{ArgVal, Args};
use cadforge_engine::sim::Sim;

fn table(cells: &[&[&str]]) -> TableData {
    let nc = cells.iter().map(|r| r.len()).max().unwrap_or(1);
    TableData { p: pt(0.0, 0.0), cols: vec![20.0; nc], rows: vec![8.0; cells.len()], cells: cells.iter().map(|r| r.iter().map(|s| s.to_string()).collect()).collect(), h: 2.5, merges: vec![], style: String::new(), title: false, rot: 0.0, link: None }
}

fn schedule() -> TableData {
    table(&[&["Item", "Qty", "Price", "Total"], &["Door", "2", "150", "=B2*C2"], &["Window", "3", "99.5", "=B3*C3"], &["", "", "Sum", "=SUM(D2:D3)"]])
}

fn tables(s: &Sim) -> Vec<(u64, TableData)> {
    s.ents().into_iter().filter_map(|e| if let Kind::Table(t) = e.kind { Some((e.id, t)) } else { None }).collect()
}

fn saved(s: &Sim) -> Vec<(String, String)> {
    s.h.borrow_mut().take_events().into_iter().filter_map(|e| if let Event::SaveFile { name, data, .. } = e { Some((name, String::from_utf8(data).unwrap())) } else { None }).collect()
}

fn args(pairs: &[(&'static str, &str)]) -> Args {
    pairs.iter().fold(Args::none(), |a, (k, v)| a.with(k, ArgVal::Str(v.to_string())))
}

#[test]
fn formula_cells_are_drawn_and_exported_as_their_values() {
    let s = Sim::new();
    let id = s.add(Entity::new(Kind::Table(schedule())));
    let shown: Vec<String> = s.doc(|d| texts_of(d.by_id(id).unwrap(), d).into_iter().map(|t| t.s).collect());
    assert!(shown.contains(&"300".to_string()) && shown.contains(&"298.5".to_string()) && shown.contains(&"598.5".to_string()), "{shown:?}");
    assert!(!shown.iter().any(|t| t.starts_with('=')), "no formula text is drawn: {shown:?}");
    s.cmd("TABLEEXPORT").click(10., -4.);
    let files = saved(&s);
    assert_eq!(files[0].1, "Item,Qty,Price,Total\r\nDoor,2,150,300\r\nWindow,3,99.5,298.5\r\n,,Sum,598.5\r\n");
    // the cell still holds the formula: change a quantity and the totals follow
    s.doc_mut(|d| d.modify(id, |e| if let Kind::Table(t) = &mut e.kind { t.cells[1][1] = "4".into() }));
    let shown: Vec<String> = s.doc(|d| texts_of(d.by_id(id).unwrap(), d).into_iter().map(|t| t.s).collect());
    assert!(shown.contains(&"600".to_string()) && shown.contains(&"898.5".to_string()), "{shown:?}");
}

#[test]
fn datalink_makes_a_linked_table_from_a_csv_file() {
    let s = Sim::new();
    s.cmd_args("DATALINK", args(&[("file", "C:/data/doors.csv"), ("text", "Mark,Width\nD1,900\nD2,\"1,200\"\n")]));
    assert!(s.prompt().contains("Select a table to link or press Enter for a new table"), "{}", s.prompt());
    s.enter();
    assert!(s.prompt().starts_with("Specify insertion point"), "{}", s.prompt());
    s.click(50., 50.);
    assert!(!s.busy(), "{}", s.log_text());
    let t = tables(&s);
    assert_eq!(t.len(), 1);
    let t = &t[0].1;
    assert_eq!(t.cells, vec![vec!["Mark", "Width"], vec!["D1", "900"], vec!["D2", "1,200"]]);
    assert_eq!(t.link.as_ref().map(|l| l.file.as_str()), Some("C:/data/doors.csv"));
    assert_eq!(t.p, pt(50., 50.));
    assert!(s.history_has("Table linked to doors.csv (3 rows, 2 columns)."), "{}", s.log_text());
    s.undo();
    assert!(tables(&s).is_empty());
}

#[test]
fn datalink_fills_an_existing_table_and_update_reads_the_file_again() {
    let s = Sim::new();
    let id = s.add(Entity::new(Kind::Table(schedule())));
    s.cmd_args("DATALINK", args(&[("file", "prices.csv"), ("text", "Item,Price\nBolt,0.25\n")]));
    s.click(10., -4.);
    let t = &tables(&s)[0].1;
    assert_eq!((t.rows.len(), t.cols.len(), t.cells[1][1].as_str()), (2, 2, "0.25"), "the table is resized to the file");
    // the file changed: Update reads it again into every table linked to it
    s.cmd_args("DATALINKUPDATE", args(&[("option", "Update"), ("file", "prices.csv"), ("text", "Item,Price\nBolt,0.30\nNut,0.10\n")]));
    let t = &tables(&s)[0].1;
    assert_eq!(t.cells, vec![vec!["Item", "Price"], vec!["Bolt", "0.30"], vec!["Nut", "0.10"]], "{}", s.log_text());
    assert!(s.history_has("Updated 1 linked table from prices.csv."));
    s.undo();
    assert_eq!(tables(&s)[0].1.rows.len(), 2, "one undo step");
    // Write saves the table back under the file's name
    s.doc_mut(|d| d.modify(id, |e| if let Kind::Table(t) = &mut e.kind { t.cells[1][1] = "=0.2+0.05".into() }));
    s.h.borrow_mut().take_events();
    s.cmd("DATALINKUPDATE").ty("Write");
    assert_eq!(saved(&s), vec![("prices.csv".to_string(), "Item,Price\r\nBolt,0.25\r\n".to_string())], "{}", s.log_text());
}

#[test]
fn without_a_file_the_host_is_asked_for_one() {
    let s = Sim::new();
    s.cmd("DATALINK");
    assert!(s.history_has("DATALINK needs a file chooser"), "{}", s.log_text());
    s.cmd("DATALINKUPDATE");
    assert!(s.history_has("No table in this drawing is linked to a data file."), "{}", s.log_text());
    s.h.borrow_mut().ui.dialogs = true;
    let mut t = schedule();
    t.link = Some(DataLink { file: "schedule.csv".into() });
    s.add(Entity::new(Kind::Table(t)));
    s.h.borrow_mut().take_events();
    s.cmd("DATALINKUPDATE").ty("Update");
    let asked: Vec<(String, Option<String>)> = s.h.borrow_mut().take_events().into_iter().filter_map(|e| if let Event::UiAction { name, args } = e { Some((name, args.str("file").map(String::from))) } else { None }).collect();
    assert_eq!(asked, vec![("DATALINKUPDATE".to_string(), Some("schedule.csv".to_string()))], "the host reads (or asks for) the linked file");
}

#[test]
fn formulas_and_links_survive_dxf_and_dwg() {
    let mut d = cadforge_core::drawing::Drawing::new("t");
    let mut t = schedule();
    t.link = Some(DataLink { file: "schedule.csv".into() });
    d.add(Entity::new(Kind::Table(t.clone())));
    let dxf_text = cadforge_core::io::write_dxf(&d);
    assert!(dxf_text.lines().any(|l| l.trim() == "598.5"), "other programs see the values: {}", dxf_text.lines().filter(|l| l.contains("598")).collect::<Vec<_>>().join(" | "));
    let back = |d: &cadforge_core::drawing::Drawing| d.entities().iter().find_map(|e| if let Kind::Table(t) = &e.kind { Some(t.clone()) } else { None });
    assert_eq!(back(&cadforge_core::io::read_dxf(&dxf_text, "t").unwrap()), Some(t.clone()), "DXF");
    let dwg = cadforge_core::io::write_dwg(&d).unwrap();
    assert_eq!(back(&cadforge_core::io::read_dwg(&dwg, "t").unwrap()), Some(t), "DWG");
}
