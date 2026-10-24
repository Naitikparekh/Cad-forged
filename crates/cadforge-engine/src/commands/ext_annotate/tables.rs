//! Table commands: TABLE, TABLEEDIT and TABLESTYLE, plus the grid operations behind them.
use super::ps;
use crate::commands::annotate::text_input;
use crate::commands::helpers::pv;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::style::TableStyle;
use cadforge_core::text::{HJust, VJust};

// ---------------------------------------------------------------- grid operations

/// Spreadsheet-style name of a cell: column letters then the row number (`A1`, `C12`, `AA3`).
fn cell_name(r: usize, c: usize) -> String {
    let mut letters = String::new();
    let mut n = c + 1;
    while n > 0 {
        letters.insert(0, (b'A' + ((n - 1) % 26) as u8) as char);
        n = (n - 1) / 26;
    }
    format!("{letters}{}", r + 1)
}

/// Make the cell text grid match the row and column counts.
fn normalize(t: &mut TableData) {
    t.cells.resize(t.rows.len(), vec![]);
    for row in &mut t.cells {
        row.resize(t.cols.len(), String::new());
    }
}

fn overlaps(m: &[usize; 4], r: &[usize; 4]) -> bool {
    m[0] <= r[2] && r[0] <= m[2] && m[1] <= r[3] && r[1] <= m[3]
}

/// The top-left cell of the merged range containing the cell (or the cell itself).
fn anchor(t: &TableData, r: usize, c: usize) -> (usize, usize) {
    t.merges.iter().find(|m| overlaps(m, &[r, c, r, c])).map_or((r, c), |m| (m[0], m[1]))
}

/// Cell of the table under a world point; merged cells resolve to their top-left cell.
fn cell_at(t: &TableData, q: Pt) -> Option<(usize, usize)> {
    let d = q - t.p;
    let (cs, sn) = (t.rot.cos(), t.rot.sin());
    let (x, y) = (d.x * cs + d.y * sn, d.x * sn - d.y * cs); // table frame: x right, y down
    let find = |sizes: &[f64], v: f64| {
        let mut end = 0.0;
        sizes.iter().position(|s| {
            end += s;
            end > v
        })
    };
    (x >= 0.0 && y >= 0.0).then_some(())?;
    Some(anchor(t, find(&t.rows, y)?, find(&t.cols, x)?))
}

/// World centre of a cell (the whole merged range for a merged cell).
fn cell_center(t: &TableData, r: usize, c: usize) -> Pt {
    let [r0, c0, r1, c1] = t.merges.iter().find(|m| m[0] == r && m[1] == c).copied().unwrap_or([r, c, r, c]);
    let sum = |v: &[f64], to: usize| v.iter().take(to).sum::<f64>();
    let (x, y) = ((sum(&t.cols, c0) + sum(&t.cols, c1 + 1)) / 2.0, (sum(&t.rows, r0) + sum(&t.rows, r1 + 1)) / 2.0);
    let (cs, sn) = (t.rot.cos(), t.rot.sin());
    pt(t.p.x + x * cs + y * sn, t.p.y + x * sn - y * cs)
}

fn insert_row(t: &mut TableData, at: usize) {
    normalize(t);
    let h = t.rows.get(at).or(t.rows.last()).copied().unwrap_or(t.h * 2.0);
    t.rows.insert(at, h);
    t.cells.insert(at, vec![String::new(); t.cols.len()]);
    for m in &mut t.merges {
        if at <= m[0] {
            m[0] += 1;
            m[2] += 1;
        } else if at <= m[2] {
            m[2] += 1;
        }
    }
}

fn insert_col(t: &mut TableData, at: usize) {
    normalize(t);
    let w = t.cols.get(at).or(t.cols.last()).copied().unwrap_or(t.h * 8.0);
    t.cols.insert(at, w);
    t.cells.iter_mut().for_each(|row| row.insert(at, String::new()));
    for m in &mut t.merges {
        if at <= m[1] {
            m[1] += 1;
            m[3] += 1;
        } else if at <= m[3] {
            m[3] += 1;
        }
    }
}

/// Shrink merged ranges after the line `at` (a row when `axis` is 0, a column when 1) has been removed.
fn shrink_merges(t: &mut TableData, at: usize, axis: usize) {
    t.merges.retain_mut(|m| {
        let (lo, hi) = (axis, axis + 2);
        if at < m[lo] {
            m[lo] -= 1;
            m[hi] -= 1;
        } else if at <= m[hi] {
            if m[lo] == m[hi] {
                return false;
            }
            m[hi] -= 1;
        }
        m[0] != m[2] || m[1] != m[3]
    });
}

fn delete_row(t: &mut TableData, r: usize) -> Result<(), &'static str> {
    if t.rows.len() < 2 {
        return Err("A table needs at least one row.");
    }
    normalize(t);
    t.rows.remove(r);
    t.cells.remove(r);
    shrink_merges(t, r, 0);
    Ok(())
}

fn delete_col(t: &mut TableData, col: usize) -> Result<(), &'static str> {
    if t.cols.len() < 2 {
        return Err("A table needs at least one column.");
    }
    normalize(t);
    t.cols.remove(col);
    t.cells.iter_mut().for_each(|row| {
        row.remove(col);
    });
    shrink_merges(t, col, 1);
    Ok(())
}

/// Merge the cells between two corner cells; ranges that partly overlap are swallowed. The texts are combined into the
/// top-left cell.
fn merge_cells(t: &mut TableData, a: (usize, usize), b: (usize, usize)) -> Result<(), &'static str> {
    normalize(t);
    let mut r = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
    loop {
        let before = r;
        for m in t.merges.iter().filter(|m| overlaps(m, &before)) {
            r = [r[0].min(m[0]), r[1].min(m[1]), r[2].max(m[2]), r[3].max(m[3])];
        }
        if r == before {
            break;
        }
    }
    if r[0] == r[2] && r[1] == r[3] {
        return Err("Pick two different cells to merge.");
    }
    let mut texts = vec![];
    for row in r[0]..=r[2] {
        for col in r[1]..=r[3] {
            let s = std::mem::take(&mut t.cells[row][col]);
            if !s.is_empty() {
                texts.push(s);
            }
        }
    }
    t.cells[r[0]][r[1]] = texts.join("\\P");
    t.merges.retain(|m| !overlaps(m, &r));
    t.merges.push(r);
    Ok(())
}

fn unmerge_cell(t: &mut TableData, r: usize, c: usize) -> bool {
    let before = t.merges.len();
    t.merges.retain(|m| !overlaps(m, &[r, c, r, c]));
    t.merges.len() != before
}

/// Turn the merged title row on or off.
fn set_title(t: &mut TableData, on: bool) {
    t.title = on;
    let span = [0, 0, 0, t.cols.len().saturating_sub(1)];
    t.merges.retain(|m| *m != span);
    if on && t.cols.len() > 1 {
        t.merges.push(span);
    }
}

fn hidden(t: &TableData, r: usize, c: usize) -> bool {
    anchor(t, r, c) != (r, c)
}

// ---------------------------------------------------------------- TABLE

async fn table(c: Ctx, _a: Args) -> CmdResult<()> {
    let style = c.doc(|d| d.settings.cur_table());
    let h = style.text_height;
    let Some(cols) = c.number(ps("Enter number of columns").default_num(4.0).integer().min(1.0).max(100.0)).await?.value() else { return Ok(()) };
    let Some(rows) = c.number(ps("Enter number of rows").default_num(3.0).integer().min(1.0).max(1000.0)).await?.value() else { return Ok(()) };
    let Some(cw) = c.distance(ps("Specify column width").default_num(8.0 * h).min(0.0).nonzero()).await?.value() else { return Ok(()) };
    let Some(rh) = c.distance(ps("Specify row height").default_num(2.0 * h).min(0.0).nonzero()).await?.value() else { return Ok(()) };
    let (nc, nr) = (cols as usize, rows as usize);
    let mut t = TableData { p: Pt::ZERO, cols: vec![cw; nc], rows: vec![rh; nr], cells: vec![vec![String::new(); nc]; nr], h, merges: vec![], style: style.name.clone(), title: false, rot: 0.0 };
    set_title(&mut t, style.title);
    let shape = t.clone();
    let Some(at) = c.point(ps("Specify insertion point").preview(move |_, p| vec![pv(Entity::new(Kind::Table(TableData { p, ..shape.clone() })))])).await?.pt() else { return Ok(()) };
    t.p = at;
    if c.keyword(ps("Enter text for the cells now").options(&["Yes", "No"]).default_str("No")).await?.kw() == Some("Yes") {
        for r in 0..nr {
            for col in 0..nc {
                if !hidden(&t, r, col) {
                    t.cells[r][col] = c.string(ps(&format!("Enter text for cell {}", cell_name(r, col))).raw()).await?.text;
                }
            }
        }
    }
    c.add(Entity::new(Kind::Table(t)));
    Ok(())
}

// ---------------------------------------------------------------- TABLEEDIT

/// Edit the table `id` through `f` (one change notification).
fn edit_table<R>(c: &Ctx, id: u64, f: impl FnOnce(&mut TableData) -> R) -> Option<R> {
    c.doc_mut(|d| {
        let mut out = None;
        d.modify(id, |e| {
            if let Kind::Table(t) = &mut e.kind {
                normalize(t);
                out = Some(f(t));
            }
        });
        out
    })
}

/// Pick a table cell: the table, row and column (`None` when Enter ends the command).
async fn pick_cell(c: &Ctx, prompt: &str) -> CmdResult<Option<(u64, usize, usize)>> {
    loop {
        let Some(q) = c.point(ps(prompt).allow_enter()).await?.pt() else { return Ok(None) };
        let hit = c.doc(|d| {
            d.entities().iter().rev().find_map(|e| match &e.kind {
                Kind::Table(t) => cell_at(t, q).map(|(r, col)| (e.id, r, col)),
                _ => None,
            })
        });
        match hit {
            Some(h) => return Ok(Some(h)),
            None => c.log_error("No table cell at that point."),
        }
    }
}

/// Edit the text of one cell (also used by DDEDIT).
pub(super) async fn edit_cell(c: &Ctx, id: u64, r: usize, col: usize) -> CmdResult<()> {
    let Some(Kind::Table(t)) = c.entity_by_id(id).map(|e| e.kind) else { return Ok(()) };
    let cur = t.cells.get(r).and_then(|row| row.get(col)).cloned().unwrap_or_default();
    c.log(&format!("Cell {}: \"{cur}\"", cell_name(r, col)));
    let spec = TextEditSpec { prompt: format!("Enter text for cell {}", cell_name(r, col)), p: cell_center(&t, r, col), h: t.h, rot: t.rot, jh: HJust::Center, jv: VJust::Middle, multi: false, w: None, initial: Some(cur), editing: None };
    let s = text_input(c, spec).await?;
    edit_table(c, id, |t| t.cells[r][col] = s);
    Ok(())
}

/// DDEDIT / TABLEEDIT entry point for a table picked at `at`.
pub(super) async fn edit_cell_at(c: &Ctx, id: u64, at: Pt) -> CmdResult<()> {
    match c.entity_by_id(id).and_then(|e| match e.kind {
        Kind::Table(t) => cell_at(&t, at),
        _ => None,
    }) {
        Some((r, col)) => edit_cell(c, id, r, col).await,
        None => {
            c.log_error("Pick a table cell.");
            Ok(())
        }
    }
}

const CELL_OPTIONS: [&str; 9] = ["Text", "insert row Above", "insert row Below", "insert column Left", "insert column Right", "Delete row", "delete Column", "Merge", "Unmerge"];

async fn tableedit(c: Ctx, _a: Args) -> CmdResult<()> {
    while let Some((id, r, col)) = pick_cell(&c, "Pick a table cell to edit").await? {
        c.log(&format!("Cell {}", cell_name(r, col)));
        let Some(opt) = c.keyword(ps("Enter an option").options(&CELL_OPTIONS).default_str("Text")).await?.kw().map(str::to_string) else { continue };
        let (end_r, end_c) = c.entity_by_id(id).and_then(|e| match e.kind {
            Kind::Table(t) => t.merges.iter().find(|m| m[0] == r && m[1] == col).map(|m| (m[2], m[3])),
            _ => None,
        }).unwrap_or((r, col));
        let result: Result<(), &str> = match opt.as_str() {
            "Text" => {
                edit_cell(&c, id, r, col).await?;
                Ok(())
            }
            "insert row Above" => edit_table(&c, id, |t| insert_row(t, r)).ok_or("Not a table."),
            "insert row Below" => edit_table(&c, id, |t| insert_row(t, end_r + 1)).ok_or("Not a table."),
            "insert column Left" => edit_table(&c, id, |t| insert_col(t, col)).ok_or("Not a table."),
            "insert column Right" => edit_table(&c, id, |t| insert_col(t, end_c + 1)).ok_or("Not a table."),
            "Delete row" => edit_table(&c, id, |t| delete_row(t, r)).unwrap_or(Err("Not a table.")),
            "delete Column" => edit_table(&c, id, |t| delete_col(t, col)).unwrap_or(Err("Not a table.")),
            "Merge" => match pick_cell(&c, "Pick the opposite cell to merge").await? {
                Some((other, r2, c2)) if other == id => edit_table(&c, id, |t| merge_cells(t, (r, col), (r2, c2))).unwrap_or(Err("Not a table.")),
                Some(_) => Err("Both cells must be in the same table."),
                None => Ok(()),
            },
            _ => match edit_table(&c, id, |t| unmerge_cell(t, r, col)) {
                Some(true) => Ok(()),
                _ => Err("The cell is not merged."),
            },
        };
        if let Err(m) = result {
            c.log_error(m);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- TABLESTYLE

async fn tablestyle(c: Ctx, _a: Args) -> CmdResult<()> {
    let cur = c.doc(|d| d.settings.cur_table());
    let name = loop {
        let n = c.string(ps("Enter table style name or [?]").default_str(&cur.name)).await?.text;
        if n != "?" {
            break n;
        }
        let names = c.doc(|d| d.settings.table_styles.keys().cloned().collect::<Vec<_>>());
        c.log(&format!("Table styles: {}", if names.is_empty() { "Standard".into() } else { names.join(", ") }));
    };
    if name.is_empty() {
        return Ok(());
    }
    let old = c.doc(|d| d.settings.table_styles.get(&name).cloned()).unwrap_or(TableStyle { name: name.clone(), ..cur });
    let Some(h) = c.distance(ps("Specify text height").default_num(old.text_height).min(0.0).nonzero()).await?.value() else { return Ok(()) };
    let title = match c.keyword(ps("Title row").options(&["Yes", "No"]).default_str(if old.title { "Yes" } else { "No" })).await?.kw() {
        Some(k) => k == "Yes",
        None => old.title,
    };
    let style = TableStyle { name: name.clone(), text_height: h, title };
    // tables using the style follow it
    let updated = c.doc_mut(|d| {
        d.change(|d| {
            d.settings.table_styles.insert(name.clone(), style.clone());
            d.settings.cur_table_style = name.clone();
            let mut n = 0;
            for e in d.all_entities_mut() {
                if let Kind::Table(t) = &mut e.kind {
                    if t.style == name || (t.style.is_empty() && name == "Standard") {
                        t.h = h;
                        set_title(t, title);
                        n += 1;
                    }
                }
            }
            n
        })
    });
    c.log(&format!("Table style \"{name}\" is current.{}", if updated > 0 { format!(" {updated} table(s) updated.") } else { String::new() }));
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(def("Draw", "TABLE", run(table)).aliases(&["TB"]).no_pickfirst().title("Table").desc("Creates an empty table object."));
    r.define(def("Annotation", "TABLEEDIT", run(tableedit)).aliases(&["TABLEDIT"]).no_pickfirst().title("Edit Table Cell").desc("Edits the text of a table cell and inserts, deletes, merges and unmerges rows, columns and cells."));
    r.define(def("Annotation", "TABLESTYLE", run(tablestyle)).aliases(&["TS"]).no_pickfirst().title("Table Style").desc("Creates, modifies or sets the current table style."));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(rows: usize, cols: usize) -> TableData {
        TableData { p: pt(0.0, 0.0), cols: vec![10.0; cols], rows: vec![5.0; rows], cells: (0..rows).map(|r| (0..cols).map(|c| cell_name(r, c)).collect()).collect(), h: 2.5, merges: vec![], style: String::new(), title: false, rot: 0.0 }
    }

    #[test]
    fn cell_names_and_lookup() {
        assert_eq!((cell_name(0, 0).as_str(), cell_name(11, 2).as_str(), cell_name(2, 26).as_str()), ("A1", "C12", "AA3"));
        let mut t = grid(3, 3);
        assert_eq!(cell_at(&t, pt(15.0, -7.0)), Some((1, 1)));
        assert_eq!(cell_at(&t, pt(-1.0, -1.0)), None);
        assert_eq!(cell_at(&t, pt(35.0, -1.0)), None);
        t.merges.push([0, 0, 1, 1]);
        assert_eq!(cell_at(&t, pt(15.0, -7.0)), Some((0, 0)), "merged cells resolve to the top-left cell");
        t.rot = PI / 2.0; // rows run to the right of a table turned a quarter turn
        t.p = pt(100.0, 100.0);
        assert_eq!(cell_at(&t, pt(107.0, 125.0)), Some((1, 2)));
        let c = cell_center(&t, 1, 2);
        assert!((c.x - 107.5).abs() < 1e-9 && (c.y - 125.0).abs() < 1e-9, "{c:?}");
    }

    #[test]
    fn inserting_and_deleting_keeps_merges_consistent() {
        let mut t = grid(3, 3);
        t.merges.push([1, 0, 2, 1]);
        insert_row(&mut t, 0);
        assert_eq!((t.rows.len(), t.cells.len(), t.merges[0]), (4, 4, [2, 0, 3, 1]), "the merge moves down with its rows");
        insert_row(&mut t, 3);
        assert_eq!(t.merges[0], [2, 0, 4, 1], "inserting inside a merge grows it");
        delete_row(&mut t, 3).unwrap();
        assert_eq!(t.merges[0], [2, 0, 3, 1]);
        delete_row(&mut t, 2).unwrap();
        assert_eq!(t.merges[0], [2, 0, 2, 1], "a merge that loses a row shrinks");
        insert_col(&mut t, 1);
        assert_eq!((t.cols.len(), t.cells[0].len(), t.merges[0]), (4, 4, [2, 0, 2, 2]));
        delete_col(&mut t, 1).unwrap();
        assert_eq!(t.merges[0], [2, 0, 2, 1]);
        delete_col(&mut t, 0).unwrap();
        assert!(t.merges.is_empty(), "a merge that shrinks to a single cell is dropped");
    }

    #[test]
    fn merge_and_unmerge() {
        let mut t = grid(3, 3);
        merge_cells(&mut t, (0, 0), (1, 1)).unwrap();
        assert_eq!(t.merges, vec![[0, 0, 1, 1]]);
        assert_eq!(t.cells[0][0], "A1\\PB1\\PA2\\PB2");
        assert!(t.cells[1][1].is_empty());
        merge_cells(&mut t, (1, 1), (2, 2)).unwrap();
        assert_eq!(t.merges, vec![[0, 0, 2, 2]], "overlapping merges are swallowed");
        assert!(merge_cells(&mut grid(2, 2), (1, 1), (1, 1)).is_err());
        assert!(unmerge_cell(&mut t, 1, 1) && t.merges.is_empty());
        assert!(!unmerge_cell(&mut t, 1, 1));
    }

    #[test]
    fn deleting_the_last_row_or_column_is_refused() {
        let mut t = grid(1, 1);
        assert!(delete_row(&mut t, 0).is_err() && delete_col(&mut t, 0).is_err());
    }

    #[test]
    fn the_title_row_is_a_merged_first_row() {
        let mut t = grid(3, 3);
        set_title(&mut t, true);
        assert_eq!(t.merges, vec![[0, 0, 0, 2]]);
        assert!(hidden(&t, 0, 1) && !hidden(&t, 1, 1));
        set_title(&mut t, false);
        assert!(t.merges.is_empty() && !t.title);
    }
}
