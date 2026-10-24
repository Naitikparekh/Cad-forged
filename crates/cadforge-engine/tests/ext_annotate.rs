//! Tests for the annotation parity batch (`commands/ext_annotate`).
use cadforge_core::dim::{dim_geometry, DimData, DimType, InspShape};
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::text::{measure_text, text_line_origins, AttDef, HJust, TextData, VJust};
use cadforge_engine::sim::Sim;

fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-6, "{a} != {b}");
}
fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn dims(s: &Sim) -> Vec<DimData> {
    s.ents()
        .into_iter()
        .filter_map(|e| match e.kind {
            Kind::Dim(d) => Some(d),
            _ => None,
        })
        .collect()
}

fn geo(s: &Sim, d: &DimData) -> cadforge_core::dim::DimGeo {
    s.doc(|doc| dim_geometry(d, &doc.settings))
}

/// A horizontal linear dimension of `p1`..`p2` with its dimension line at height `y`.
fn lin(s: &Sim, x1: f64, x2: f64, y: f64) -> u64 {
    let mut d = DimData::new(DimType::Lin);
    (d.p1, d.p2, d.dp, d.ang, d.ds) = (
        Some(p(x1, 0.)),
        Some(p(x2, 0.)),
        Some(p((x1 + x2) / 2., y)),
        Some(0.),
        Some("Standard".into()),
    );
    s.add(Entity::dim(d))
}

fn dim_of(s: &Sim, id: u64) -> DimData {
    match s.doc(|d| d.by_id(id).cloned()).unwrap().kind {
        Kind::Dim(d) => d,
        k => panic!("{k:?}"),
    }
}

// ---------------------------------------------------------------- dimensions

#[test]
fn dimarc_measures_the_arc_length_of_an_arc() {
    let s = Sim::new();
    s.add(Entity::arc(p(0., 0.), 10., 0., PI / 2.));
    let m = 10.0 * std::f64::consts::FRAC_1_SQRT_2;
    s.cmd("DIMARC").click(m, m);
    assert!(
        s.prompt().contains("Specify arc length dimension location"),
        "{}",
        s.prompt()
    );
    s.click(14., 14.);
    assert!(!s.busy());
    let d = &dims(&s)[0];
    assert_eq!(d.kind, DimType::Arc);
    let g = geo(&s, d);
    near(g.measured, 10.0 * PI / 2.0);
    assert!(g.text.s.ends_with("15.71"), "{}", g.text.s);
    assert_eq!(s.doc(|d| d.undo_depth()), 1, "one undo step");
}

#[test]
fn dimarc_partial_and_polyline_arc_segments() {
    let s = Sim::new();
    s.add(Entity::arc(p(0., 0.), 10., 0., PI / 2.));
    s.cmd("DIMARC")
        .click(7.0710678, 7.0710678)
        .ty("p")
        .click(10. * (PI / 6.).cos(), 10. * (PI / 6.).sin())
        .click(10. * (PI / 3.).cos(), 10. * (PI / 3.).sin())
        .click(14., 14.);
    assert!(!s.busy());
    near(geo(&s, &dims(&s)[0]).measured, 10.0 * PI / 6.0);
    // an arc segment of a polyline (bulge tan(90°/4))
    let s = Sim::new();
    s.add(Entity::pline(
        vec![
            Vertex::new(10., 0., (PI / 8.).tan()),
            Vertex::new(0., 10., 0.),
        ],
        false,
    ));
    s.cmd("DIMARC").click(7.0710678, 7.0710678).click(14., 14.);
    near(geo(&s, &dims(&s)[0]).measured, 10.0 * PI / 2.0);
    // a line is not an arc
    let s = Sim::new();
    s.add(Entity::line(p(0., 0.), p(10., 0.)));
    s.cmd("DIMARC").click(5., 0.);
    assert!(s.busy(), "a line cannot be picked: {}", s.prompt());
}

#[test]
fn qdim_creates_a_chain_of_linear_dimensions() {
    let s = Sim::new();
    for x in [0., 10., 25.] {
        s.add(Entity::line(p(x, 0.), p(x, 5.)));
    }
    s.cmd("QDIM").ty("all").enter();
    assert!(
        s.prompt().contains("Specify dimension line position, or")
            && s.prompt().contains("<Continuous>"),
        "{}",
        s.prompt()
    );
    s.click(12., -8.);
    assert!(!s.busy());
    let d = dims(&s);
    assert_eq!(
        d.len(),
        2,
        "three vertical lines make two continuous dimensions"
    );
    assert_eq!(
        d.iter().map(|x| geo(&s, x).measured).collect::<Vec<_>>(),
        vec![10., 15.]
    );
    assert!(d
        .iter()
        .all(|x| x.kind == DimType::Lin && x.ang == Some(0.)));
}

#[test]
fn qdim_types_baseline_staggered_ordinate_and_radius() {
    let build = |opt: &str| {
        let s = Sim::new();
        for x in [0., 10., 25., 40.] {
            s.add(Entity::line(p(x, 0.), p(x, 5.)));
        }
        s.cmd("QDIM").ty("all").enter().ty(opt).click(20., -8.);
        assert!(!s.busy(), "{opt}: {}", s.prompt());
        s
    };
    let s = build("baseline");
    let d = dims(&s);
    assert_eq!(d.len(), 3);
    let ys: Vec<f64> = d.iter().map(|x| x.dp.unwrap().y).collect();
    assert!(
        ys[1] < ys[0] && ys[2] < ys[1],
        "baseline dimensions step away from the geometry: {ys:?}"
    );
    assert_eq!(
        d.iter().map(|x| geo(&s, x).measured).collect::<Vec<_>>(),
        vec![10., 25., 40.]
    );
    let s = build("staggered");
    let ys: Vec<f64> = dims(&s).iter().map(|x| x.dp.unwrap().y).collect();
    assert_eq!(ys.len(), 3);
    assert!((ys[0] - ys[2]).abs() < 1e-9 && (ys[0] - ys[1]).abs() > 1.0);
    let s = build("ordinate");
    let d = dims(&s);
    assert_eq!(d.len(), 4);
    assert!(d.iter().all(|x| x.kind == DimType::Ord && x.xd));
    assert_eq!(
        d.iter().map(|x| geo(&s, x).measured).collect::<Vec<_>>(),
        vec![0., 10., 25., 40.]
    );
    // radius / diameter: one dimension per circle
    let s = Sim::new();
    for x in [0., 30., 60.] {
        s.add(Entity::circle(p(x, 0.), 5.));
    }
    s.cmd("QDIM")
        .ty("all")
        .enter()
        .ty("diameter")
        .click(30., 20.);
    let d = dims(&s);
    assert_eq!(d.len(), 3);
    assert!(d.iter().all(|x| x.kind == DimType::Dia));
    near(geo(&s, &d[1]).measured, 10.);
    let s = Sim::new();
    for x in [0., 30.] {
        s.add(Entity::circle(p(x, 0.), 5.));
    }
    s.cmd("QDIM").ty("all").enter().ty("r").click(15., 20.);
    assert_eq!(dims(&s).len(), 2);
}

#[test]
fn qdim_edit_removes_a_dimension_point() {
    let s = Sim::new();
    for x in [0., 10., 25.] {
        s.add(Entity::line(p(x, 0.), p(x, 5.)));
    }
    s.cmd("QDIM").ty("all").enter().ty("e");
    assert!(
        s.prompt().contains("Indicate dimension point to remove"),
        "{}",
        s.prompt()
    );
    s.click(10., 0.).click(10., 5.).enter(); // both ends of the middle line
    s.click(12., -8.);
    let d = dims(&s);
    assert_eq!(
        d.len(),
        1,
        "removing the middle position leaves one dimension"
    );
    near(geo(&s, &d[0]).measured, 25.);
}

#[test]
fn dimjogged_puts_the_arrow_on_the_circle_along_the_dimension_line() {
    let s = Sim::new();
    s.add(Entity::circle(p(0., 0.), 10.));
    s.cmd("DIMJOGGED").click(10., 0.);
    assert!(
        s.prompt().contains("Specify center location override"),
        "{}",
        s.prompt()
    );
    s.click(-5., 2.);
    assert!(
        s.prompt().contains("Specify dimension line location"),
        "{}",
        s.prompt()
    );
    s.click(20., 6.);
    assert!(
        s.prompt().contains("Specify jog location"),
        "{}",
        s.prompt()
    );
    s.click(2., 3.);
    assert!(!s.busy());
    let d = &dims(&s)[0];
    assert_eq!(d.kind, DimType::Jog);
    let on = d.pt.unwrap();
    near(dist(on, p(0., 0.)), 10.);
    near(cross(on - p(-5., 2.), p(20., 6.) - p(-5., 2.)), 0.);
    let g = geo(&s, d);
    near(g.measured, 10.);
    assert_eq!(g.text.s, "R10.00");
}

#[test]
fn dimordinate_chooses_the_datum_from_the_leader_direction() {
    let s = Sim::new();
    s.cmd("DIMORDINATE").click(30., 20.).click(30., 40.);
    assert!(!s.busy());
    let d = dims(&s)[0].clone();
    assert!(d.xd, "a vertical leader dimensions the X ordinate");
    near(geo(&s, &d).measured, 30.);
    s.cmd("DOR").click(30., 20.).click(60., 21.);
    let d = dims(&s)[1].clone();
    assert!(!d.xd);
    near(geo(&s, &d).measured, 20.);
    s.cmd("DIMORDINATE").click(30., 20.).ty("y").click(30., 40.);
    assert!(!dims(&s)[2].xd, "Ydatum overrides the leader direction");
}

#[test]
fn dimbreak_cuts_the_dimension_line_where_other_objects_cross() {
    let s = Sim::new();
    let id = lin(&s, 0., 100., -10.);
    s.add(Entity::line(p(30., -20.), p(30., 5.)));
    let segs = |s: &Sim| geo(s, &dim_of(s, id)).segs.len();
    assert_eq!(segs(&s), 3);
    s.cmd("DIMBREAK").click(70., -10.).enter();
    assert!(!s.busy());
    assert_eq!(dim_of(&s, id).brk.len(), 1);
    assert_eq!(segs(&s), 4);
    assert!(s.history_has("1 dimension break added"), "{}", s.log_text());
    // Remove restores the dimension line
    s.cmd("DIMBREAK").click(70., -10.).ty("r");
    assert!(!s.busy());
    assert!(dim_of(&s, id).brk.is_empty());
    assert_eq!(segs(&s), 3);
    // Manual breaks between two points
    s.cmd("DIMBREAK")
        .click(70., -10.)
        .ty("m")
        .click(60., -10.)
        .click(65., -10.);
    assert_eq!(dim_of(&s, id).brk.len(), 1);
    let (i, a, b) = dim_of(&s, id).brk[0];
    assert_eq!(i, 2);
    near(b - a, 5.);
}

#[test]
fn dimcenter_draws_a_center_mark_or_centerlines() {
    let s = Sim::new();
    s.add(Entity::circle(p(10., 10.), 4.));
    s.cmd("DIMCENTER").click(14., 10.);
    assert!(!s.busy());
    let lines: Vec<Entity> = s
        .ents()
        .into_iter()
        .filter(|e| matches!(e.kind, Kind::Line { .. }))
        .collect();
    assert_eq!(lines.len(), 2, "a cross");
    assert!(lines[0].grp.is_some() && lines[0].grp == lines[1].grp);
    if let Kind::Line { a, b } = lines[0].kind {
        near(dist(a, b), 2.0 * 2.5); // DIMCEN: 2.5 either side of the centre
    }
    // a negative DIMCEN draws centerlines that reach beyond the circle
    let s = Sim::new();
    s.add(Entity::circle(p(0., 0.), 4.));
    s.doc_mut(|d| d.settings.dim_styles.get_mut("Standard").unwrap().cen = -2.5);
    s.cmd("DCE").click(4., 0.);
    assert_eq!(s.n(), 1 + 6);
    // DIMCEN 0: nothing to draw
    let s = Sim::new();
    s.add(Entity::circle(p(0., 0.), 4.));
    s.doc_mut(|d| d.settings.dim_styles.get_mut("Standard").unwrap().cen = 0.);
    s.cmd("DIMCENTER").click(4., 0.);
    assert_eq!(s.n(), 1);
    assert!(s.history_has("DIMCEN"));
}

#[test]
fn dimspace_spaces_parallel_dimensions_evenly() {
    let s = Sim::new();
    let base = lin(&s, 0., 10., -10.);
    let a = lin(&s, 0., 20., -14.);
    let b = lin(&s, 0., 30., -30.);
    let up = lin(&s, 0., 15., 4.);
    s.cmd("DIMSPACE").click(5., -10.);
    s.ty("all").enter();
    assert!(
        s.prompt().contains("Enter value or [Auto]") && s.prompt().contains("<Auto>"),
        "{}",
        s.prompt()
    );
    s.ty("5");
    assert!(!s.busy());
    let y = |id| dim_of(&s, id).dp.unwrap().y;
    near(y(base), -10.);
    near(y(a), -15.);
    near(y(b), -20.);
    near(y(up), -5.); // above the base: 5 units on the other side of it
                      // Auto spacing is twice the text height, 0 aligns them
    s.cmd("DIMSPACE").click(5., -10.).ty("all").enter().enter();
    near(y(a), -10. - 5.);
    s.cmd("DIMSPACE").click(5., -10.).ty("all").enter().ty("0");
    near(y(b), -10.);
}

#[test]
fn dimedit_home_new_rotate_and_oblique() {
    let s = Sim::new();
    let id = lin(&s, 0., 100., -10.);
    s.cmd("DIMEDIT")
        .ty("n")
        .ty("<> TYP")
        .click(50., -10.)
        .enter();
    assert_eq!(geo(&s, &dim_of(&s, id)).text.s, "100.00 TYP");
    s.cmd("DED").ty("rotate").ty("90").click(30., -10.).enter();
    near(geo(&s, &dim_of(&s, id)).text.rot, PI / 2.);
    s.cmd("DIMEDIT").ty("o").click(30., -10.).enter().ty("60");
    let d = dim_of(&s, id);
    near(d.obl.unwrap(), rad(60.) - PI / 2.);
    let g = geo(&s, &d);
    near(g.measured, 100.);
    if let Seg::L { a, b, .. } = g.segs[0] {
        near(deg(norm_angle(angle_of(a, b))) % 180., 60.);
    }
    s.cmd("DIMEDIT").ty("o").click(30., -10.).enter().enter();
    assert!(dim_of(&s, id).obl.is_none(), "Enter removes the obliquing");
    s.doc_mut(|d| {
        if let Some(Entity {
            kind: Kind::Dim(dd),
            ..
        }) = d.by_id_mut(id)
        {
            dd.tp = Some(p(80., -20.));
        }
    });
    s.cmd("DIMEDIT").enter().click(30., -10.).enter();
    let d = dim_of(&s, id);
    assert!(
        d.tp.is_none() && d.tr.is_none(),
        "Home restores the text position and angle"
    );
    assert_eq!(s.doc(|d| d.undo_depth()), 5);
}

#[test]
fn dimtedit_moves_and_rotates_the_text() {
    let s = Sim::new();
    let id = lin(&s, 0., 100., -10.);
    let home = geo(&s, &dim_of(&s, id)).text.p;
    s.cmd("DIMTEDIT").click(20., -10.).click(70., -25.);
    assert!(!s.busy());
    assert_eq!(dim_of(&s, id).tp, Some(p(70., -25.)));
    s.cmd("DIMTEDIT").click(20., -10.).ty("left");
    let left = geo(&s, &dim_of(&s, id)).text.p;
    assert!(
        left.x < home.x - 10. && (left.y - home.y).abs() < 1e-9,
        "{left:?} {home:?}"
    );
    s.cmd("DIMTEDIT").click(20., -10.).ty("right");
    assert!(geo(&s, &dim_of(&s, id)).text.p.x > home.x + 10.);
    s.cmd("DIMTEDIT").click(20., -10.).ty("angle").ty("45");
    near(geo(&s, &dim_of(&s, id)).text.rot, PI / 4.);
    s.cmd("DIMTEDIT").click(20., -10.).ty("home");
    let d = dim_of(&s, id);
    assert!(d.tp.is_none() && d.tr.is_none());
}

#[test]
fn diminspect_adds_and_removes_the_inspection_frame() {
    let s = Sim::new();
    let id = lin(&s, 0., 100., -10.);
    let plain = geo(&s, &dim_of(&s, id)).segs.len();
    s.cmd("DIMINSPECT").click(20., -10.).enter();
    s.enter().enter().ty("A7").ty("75"); // Add, Round, label, rate
    assert!(!s.busy(), "{}", s.prompt());
    let d = dim_of(&s, id);
    let i = d.insp.clone().unwrap();
    assert_eq!(
        (i.label.as_str(), i.rate, i.shape),
        ("A7", Some(75), InspShape::Round)
    );
    let g = geo(&s, &d);
    assert_eq!(g.text.s, "A7 | 100.00 | 75%");
    assert_eq!(g.segs.len(), plain + 4);
    s.cmd("DIMINSPECT").click(20., -10.).enter().ty("remove");
    assert!(dim_of(&s, id).insp.is_none());
}

#[test]
fn new_dimension_commands_are_registered_with_their_aliases() {
    let s = Sim::new();
    let r = s.registry();
    for (typed, name) in [
        ("DAR", "DIMARC"),
        ("DJO", "DIMJOGGED"),
        ("DOR", "DIMORDINATE"),
        ("DCE", "DIMCENTER"),
        ("DED", "DIMEDIT"),
        ("DIMTED", "DIMTEDIT"),
        ("QDIM", "QDIM"),
        ("DIMBREAK", "DIMBREAK"),
        ("DIMSPACE", "DIMSPACE"),
        ("DIMINSPECT", "DIMINSPECT"),
    ] {
        assert_eq!(
            r.find(typed).map(|d| d.name.clone()).as_deref(),
            Some(name),
            "{typed}"
        );
    }
    assert_eq!(
        r.find("ED").map(|d| d.name.clone()).as_deref(),
        Some("TEXTEDIT"),
        "ED stays with TEXTEDIT"
    );
}

// ---------------------------------------------------------------- tables

fn table_of(s: &Sim) -> TableData {
    s.ents()
        .into_iter()
        .find_map(|e| match e.kind {
            Kind::Table(t) => Some(t),
            _ => None,
        })
        .expect("a table")
}

/// A 3 x 2 table (columns x rows) of 20 x 6 cells whose top-left corner is at (0, 100).
fn make_table(s: &Sim) {
    s.cmd("TABLE")
        .ty("3")
        .ty("2")
        .ty("20")
        .ty("6")
        .click(0., 100.)
        .enter();
    assert!(!s.busy(), "{}", s.prompt());
}

#[test]
fn table_creates_a_grid_of_the_requested_size() {
    let s = Sim::new();
    s.cmd("TABLE");
    assert!(
        s.prompt().contains("Enter number of columns"),
        "{}",
        s.prompt()
    );
    s.ty("3");
    assert!(
        s.prompt().contains("Enter number of rows"),
        "{}",
        s.prompt()
    );
    s.ty("2").ty("20").ty("6");
    assert!(
        s.prompt().contains("Specify insertion point"),
        "{}",
        s.prompt()
    );
    s.click(10., 50.);
    assert!(
        s.prompt().contains("Enter text for the cells now"),
        "{}",
        s.prompt()
    );
    s.ty("yes");
    for (i, text) in ["Part", "Qty", "", "Bolt", "4", "M6"].iter().enumerate() {
        assert!(
            s.prompt().contains(&format!(
                "Enter text for cell {}{}",
                ["A", "B", "C"][i % 3],
                i / 3 + 1
            )),
            "{}",
            s.prompt()
        );
        s.ty(text);
    }
    assert!(!s.busy());
    let t = table_of(&s);
    assert_eq!(
        (t.cols.clone(), t.rows.clone(), t.p),
        (vec![20.; 3], vec![6.; 2], p(10., 50.))
    );
    assert_eq!(
        t.cells,
        vec![vec!["Part", "Qty", ""], vec!["Bolt", "4", "M6"]]
    );
    assert!(t.merges.is_empty() && !t.title);
    assert_eq!(s.n(), 1);
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
    // the grid is drawn: 3 horizontal and 4 vertical lines
    let segs = s.doc(|d| segs_of(&s.ent(0), d).len());
    assert_eq!(segs, 3 + 4);
}

#[test]
fn table_defaults_and_validation() {
    let s = Sim::new();
    s.cmd("TB")
        .enter()
        .enter()
        .enter()
        .enter()
        .click(0., 0.)
        .enter();
    let t = table_of(&s);
    assert_eq!((t.cols.len(), t.rows.len()), (4, 3));
    assert_eq!((t.cols[0], t.rows[0]), (20., 5.));
    assert!(t.cells.iter().flatten().all(|c| c.is_empty()));
    let s = Sim::new();
    s.cmd("TABLE").ty("0");
    assert!(
        s.prompt().contains("Enter number of columns"),
        "0 columns is rejected"
    );
    s.ty("2.5");
    assert!(
        s.prompt().contains("Enter number of columns"),
        "{}",
        s.prompt()
    );
    s.esc();
    assert_eq!(s.n(), 0);
}

#[test]
fn tablestyle_sets_text_height_and_the_title_row() {
    let s = Sim::new();
    s.cmd("TABLESTYLE").ty("Parts");
    assert!(s.prompt().contains("Specify text height"), "{}", s.prompt());
    s.ty("3");
    assert!(s.prompt().contains("Title row"), "{}", s.prompt());
    s.ty("yes");
    assert!(!s.busy());
    assert_eq!(s.doc(|d| d.settings.cur_table().name), "Parts");
    s.cmd("TABLE")
        .ty("3")
        .ty("3")
        .enter()
        .enter()
        .click(0., 0.)
        .enter();
    let t = table_of(&s);
    assert_eq!(
        (t.h, t.style.as_str(), t.title, t.merges.clone()),
        (3., "Parts", true, vec![[0, 0, 0, 2]])
    );
    // modifying the style updates the tables that use it
    s.cmd("TS").ty("Parts").ty("4").ty("no");
    let t = table_of(&s);
    assert_eq!((t.h, t.title, t.merges.len()), (4., false, 0));
    assert!(
        s.log_text().contains("1 table(s) updated"),
        "{}",
        s.log_text()
    );
    // the style survives a save
    let json = s.doc(|d| d.to_json_string());
    let back = cadforge_core::drawing::Drawing::from_json_str(&json, None).unwrap();
    assert_eq!(back.settings.table_styles["Parts"].text_height, 4.);
}

#[test]
fn tableedit_edits_cell_text() {
    let s = Sim::new();
    make_table(&s);
    s.cmd("TABLEEDIT").click(30., 97.); // B1
    assert!(
        s.prompt().contains("Enter an option") && s.prompt().contains("<Text>"),
        "{}",
        s.prompt()
    );
    s.enter();
    assert!(
        s.prompt().contains("Enter text for cell B1"),
        "{}",
        s.prompt()
    );
    s.ty("Qty").enter(); // back at the cell prompt: Enter ends the command
    assert!(!s.busy());
    assert_eq!(table_of(&s).cells[0][1], "Qty");
    // a point outside the table is refused
    s.cmd("TABLEEDIT").click(200., 200.);
    assert!(
        s.busy() && s.prompt().contains("Pick a table cell"),
        "{}",
        s.prompt()
    );
    s.esc();
}

#[test]
fn tableedit_inserts_and_deletes_rows_and_columns() {
    let s = Sim::new();
    make_table(&s);
    s.cmd("TABLEEDIT")
        .click(10., 97.)
        .ty("insert row above")
        .enter();
    assert_eq!(table_of(&s).rows.len(), 3, "a row was inserted");
    s.cmd("TABLEEDIT")
        .click(10., 97.)
        .ty("insert row below")
        .enter();
    assert_eq!(table_of(&s).rows.len(), 4);
    s.cmd("TABLEEDIT")
        .click(10., 97.)
        .ty("insert column left")
        .enter();
    s.cmd("TABLEEDIT").click(10., 97.).ty("R").enter();
    let t = table_of(&s);
    assert_eq!((t.cols.len(), t.rows.len()), (5, 4));
    assert!(t.cells.iter().all(|r| r.len() == 5));
    s.cmd("TABLEEDIT")
        .click(10., 97.)
        .ty("delete row")
        .click(10., 97.)
        .ty("delete column")
        .enter();
    let t = table_of(&s);
    assert_eq!((t.cols.len(), t.rows.len()), (4, 3));
    assert_eq!(s.doc(|d| d.undo_depth()), 6, "one undo step per command");
    s.undo();
    assert_eq!(table_of(&s).cols.len(), 5);
}

#[test]
fn tableedit_merges_and_unmerges_cells() {
    let s = Sim::new();
    make_table(&s);
    s.cmd("TABLEEDIT")
        .click(10., 97.)
        .ty("merge")
        .click(30., 97.)
        .enter();
    assert_eq!(table_of(&s).merges, vec![[0, 0, 0, 1]]);
    // picking the hidden half of a merged cell addresses the merged cell
    s.cmd("TABLEEDIT").click(30., 97.).ty("unmerge").enter();
    assert!(table_of(&s).merges.is_empty());
    s.cmd("TABLEEDIT").click(30., 97.).ty("unmerge").enter();
    assert!(s.history_has("not merged"), "{}", s.log_text());
}

// ---------------------------------------------------------------- text

fn text_of(s: &Sim, id: u64) -> TextData {
    match s.doc(|d| d.by_id(id).cloned()).unwrap().kind {
        Kind::Text(t) => t,
        k => panic!("{k:?}"),
    }
}

/// One of everything FIND searches: text, table cells, a leader, a dimension override and a block attribute.
fn find_fixture(s: &Sim) {
    s.add(Entity::text(p(0., 0.), 2.5, "Door DOOR hinge"));
    s.add(Entity::new(Kind::Table(TableData {
        p: p(0., 100.),
        cols: vec![20., 20.],
        rows: vec![6.],
        cells: vec![vec!["Door".into(), "doors".into()]],
        h: 2.5,
        merges: vec![],
        style: String::new(),
        title: false,
        rot: 0.,
    })));
    s.add(Entity::leader(
        vec![p(50., 0.), p(60., 10.)],
        "Back door",
        2.5,
    ));
    let mut dd = DimData::new(DimType::Lin);
    (dd.p1, dd.p2, dd.dp, dd.ang, dd.txt) = (
        Some(p(0., 30.)),
        Some(p(10., 30.)),
        Some(p(5., 25.)),
        Some(0.),
        Some("<> DOOR".into()),
    );
    s.add(Entity::dim(dd));
    let mut ins = Entity::insert("B", p(80., 0.));
    if let Kind::Insert(i) = &mut ins.kind {
        i.attribs = vec![AttribData {
            tag: "T".into(),
            t: TextData::new(p(80., 5.), 2.5, "front door"),
            hidden: false,
        }];
    }
    s.add(ins);
}

fn all_strings(s: &Sim) -> Vec<String> {
    let mut out = vec![];
    for e in s.ents() {
        match e.kind {
            Kind::Text(t) => out.push(t.s),
            Kind::Table(t) => out.extend(t.cells.into_iter().flatten()),
            Kind::Leader { s, .. } => out.push(s),
            Kind::Dim(d) => out.extend(d.txt),
            Kind::Insert(i) => out.extend(i.attribs.into_iter().map(|a| a.t.s)),
            _ => {}
        }
    }
    out
}

#[test]
fn find_replaces_text_everywhere_and_reports_the_count() {
    let s = Sim::new();
    find_fixture(&s);
    s.cmd("FIND").ty("door");
    assert!(
        s.prompt().contains("Enter replacement text"),
        "{}",
        s.prompt()
    );
    s.ty("window");
    assert!(
        s.prompt()
            .contains("[Case/Whole word/Select objects/Find/Replace all] <Replace all>"),
        "{}",
        s.prompt()
    );
    s.enter();
    assert!(!s.busy());
    assert_eq!(
        all_strings(&s),
        vec![
            "window window hinge",
            "window",
            "windows",
            "Back window",
            "<> window",
            "front window"
        ]
    );
    assert!(
        s.history_has("7 occurrences replaced in 5 objects."),
        "{}",
        s.log_text()
    );
    assert_eq!(s.doc(|d| d.undo_depth()), 1, "one undo step");
    s.undo();
    assert!(all_strings(&s).contains(&"Door DOOR hinge".to_string()));
}

#[test]
fn find_options_case_whole_word_and_find_only() {
    let s = Sim::new();
    find_fixture(&s);
    s.cmd("FIND")
        .ty("door")
        .ty("window")
        .ty("case")
        .ty("whole word")
        .enter();
    assert!(
        s.history_has("Match case: on") && s.history_has("Find whole words only: on"),
        "{}",
        s.log_text()
    );
    assert_eq!(
        all_strings(&s)[0],
        "Door DOOR hinge",
        "no exact-case word matches"
    );
    assert!(
        all_strings(&s).contains(&"front window".to_string())
            && all_strings(&s).contains(&"Back window".to_string())
    );
    assert!(
        s.history_has("2 occurrences replaced in 2 objects."),
        "{}",
        s.log_text()
    );
    // an empty replacement only finds
    let s = Sim::new();
    find_fixture(&s);
    let before = all_strings(&s);
    s.cmd("FIND").ty("DOOR").enter();
    assert!(s.prompt().contains("<Find>"), "{}", s.prompt());
    s.enter();
    assert_eq!(all_strings(&s), before);
    assert!(
        s.history_has("7 occurrences of \"DOOR\" found in 5 objects."),
        "{}",
        s.log_text()
    );
    // Replace all with an empty replacement deletes the text
    s.cmd("FIND").ty("hinge").enter().ty("replace all");
    assert_eq!(all_strings(&s)[0], "Door DOOR ");
}

#[test]
fn find_can_be_limited_to_a_selection_and_skips_locked_layers() {
    let s = Sim::new();
    let a = s.add(Entity::text(p(0., 0.), 2.5, "door"));
    let b = s.add(Entity::text(p(0., 20.), 2.5, "door"));
    s.doc_mut(|d| {
        d.add_layer("Frozen").unwrap();
        d.layer_mut("Frozen").unwrap().locked = true;
        d.by_id_mut(b).unwrap().layer = "Frozen".into();
    });
    s.cmd("FIND").ty("door").ty("gate").ty("select");
    s.click(1., 1.).enter();
    s.enter();
    assert_eq!(
        (text_of(&s, a).s.as_str(), text_of(&s, b).s.as_str()),
        ("gate", "door")
    );
    let c = s.add(Entity::text(p(0., 40.), 2.5, "door"));
    s.cmd("FIND").ty("door").ty("gate").enter();
    assert_eq!(text_of(&s, c).s, "gate");
    assert_eq!(text_of(&s, b).s, "door", "locked layers are not changed");
    assert!(
        s.history_has("1 object on locked layers skipped."),
        "{}",
        s.log_text()
    );
}

#[test]
fn ddedit_edits_text_attribute_definitions_and_table_cells() {
    let s = Sim::new();
    let t = s.add(Entity::text(p(0., 0.), 2.5, "old"));
    s.cmd("DDEDIT").click(1., 1.);
    assert!(s.prompt().contains("Enter new text"), "{}", s.prompt());
    s.ty("new");
    assert!(
        s.prompt().contains("Select an annotation object"),
        "DDEDIT keeps asking: {}",
        s.prompt()
    );
    s.enter();
    assert!(!s.busy());
    assert_eq!(text_of(&s, t).s, "new");
    // attribute definition: tag, prompt, default value
    let mut td = TextData::new(p(0., 30.), 2.5, "dflt");
    td.att = Some(AttDef {
        tag: "TAG".into(),
        prompt: "Part?".into(),
        invisible: false,
        constant: false,
        verify: false,
        preset: false,
    });
    let a = s.add(Entity::new(Kind::Text(td)));
    s.cmd("DDEDIT").click(1., 31.);
    assert!(
        s.prompt().contains("Enter new tag") && s.prompt().contains("<TAG>"),
        "{}",
        s.prompt()
    );
    s.ty("NO").enter().ty("v2").enter();
    let t = text_of(&s, a);
    let att = t.att.clone().unwrap();
    assert_eq!(
        (att.tag.as_str(), att.prompt.as_str(), t.s.as_str()),
        ("NO", "Part?", "v2")
    );
    // a table cell, picked next to a grid line
    make_table(&s);
    s.cmd("DDEDIT").click(19.9, 97.);
    assert!(
        s.prompt().contains("Enter text for cell A1"),
        "{}",
        s.prompt()
    );
    s.ty("Cell").enter();
    assert_eq!(table_of(&s).cells[0][0], "Cell");
}

#[test]
fn spell_reports_unknown_words() {
    let s = Sim::new();
    s.add(Entity::text(
        p(0., 0.),
        2.5,
        "The quikc brown dimension xyzzy and PART-12 M6 doors",
    ));
    s.add(Entity::mtext(
        p(0., 20.),
        2.5,
        "Second quikc line\\Pwith a mispeling",
        80.,
    ));
    s.cmd("SPELL").enter();
    assert!(!s.busy());
    let log = s.log_text();
    assert!(
        log.contains("Unknown word: \"quikc\" (2 times, first in Text)"),
        "{log}"
    );
    assert!(
        log.contains("\"xyzzy\"")
            && log.contains("\"mispeling\"")
            && !log.contains("\"brown\"")
            && !log.contains("\"doors\""),
        "{log}"
    );
    assert!(
        s.history_has("Spelling check complete: 3 unknown words."),
        "{log}"
    );
    // only the selected objects are checked
    s.cmd("SPELL").click(1., 1.).enter();
    assert!(s.last_log().contains("2 unknown words"), "{}", s.last_log());
}

#[test]
fn field_inserts_evaluated_text() {
    let s = Sim::new();
    s.doc_mut(|d| d.file_name = Some("C:\\work\\Part-1.dxf".into()));
    s.cmd("FIELD").ty("filename").click(0., 0.);
    assert!(!s.busy());
    assert_eq!(text_of(&s, s.ent(0).id).s, "Part-1.dxf");
    s.cmd("FIELD").enter();
    assert!(s.prompt().contains("<yyyy-MM-dd>"), "{}", s.prompt());
    s.enter().click(0., 10.);
    let date = text_of(&s, s.ent(1).id).s;
    let parts: Vec<&str> = date.split('-').collect();
    assert!(
        parts.len() == 3
            && parts[0].len() == 4
            && parts[0].parse::<u32>().unwrap() >= 2024
            && parts[1].len() == 2
            && parts[2].len() == 2,
        "{date}"
    );
    s.cmd("FIELD").enter().ty("yyyy").click(0., 20.);
    assert_eq!(text_of(&s, s.ent(2).id).s.len(), 4);
    s.add(Entity::circle(p(50., 0.), 2.));
    s.cmd("FIELD")
        .ty("object")
        .click(52., 0.)
        .enter()
        .click(0., 30.);
    assert_eq!(text_of(&s, s.ent(4).id).s, "12.5664");
    s.cmd("FIELD")
        .ty("object")
        .click(52., 0.)
        .ty("diameter")
        .click(0., 40.);
    assert_eq!(text_of(&s, s.ent(5).id).s, "4.0000");
    s.cmd("FIELD")
        .ty("object")
        .click(52., 0.)
        .ty("length")
        .click(0., 50.);
    assert_eq!(text_of(&s, s.ent(6).id).s, "12.5664");
    s.cmd("FIELD").ty("object").click(52., 0.).ty("contents");
    assert!(s.history_has("has no Contents value"), "{}", s.log_text());
    assert!(!s.busy());
}

#[test]
fn scaletext_scales_about_the_chosen_base_point() {
    let s = Sim::new();
    let id = s.add(Entity::text(p(10., 10.), 2.5, "Hello"));
    let w = measure_text("Hello", 2.5);
    s.cmd("SCALETEXT").click(11., 11.).enter();
    assert!(s.prompt().contains("<Existing>"), "{}", s.prompt());
    s.enter();
    assert!(
        s.prompt().contains("Specify new height") && s.prompt().contains("<2.5>"),
        "{}",
        s.prompt()
    );
    s.ty("5");
    let t = text_of(&s, id);
    assert_eq!(
        (t.h, t.p),
        (5., p(10., 10.)),
        "scaled about its own insertion point"
    );
    // half size about the middle of the baseline: that point stays where it is
    s.cmd("SCALETEXT")
        .click(11., 11.)
        .enter()
        .ty("center")
        .ty("s")
        .ty("0.5");
    let t = text_of(&s, id);
    near(t.h, 2.5);
    near(t.p.x, 10. + w / 2.);
    near(t.p.y, 10.);
    assert_eq!(s.doc(|d| d.undo_depth()), 2);
}

#[test]
fn scaletext_match_object_copies_the_height() {
    let s = Sim::new();
    let a = s.add(Entity::text(p(0., 0.), 2.5, "A"));
    let b = s.add(Entity::text(p(0., 20.), 7., "B"));
    s.cmd("SCALETEXT").click(0.5, 1.).enter().enter().ty("m");
    s.click(0.5, 21.);
    assert!(!s.busy());
    near(text_of(&s, a).h, 7.);
    near(text_of(&s, b).h, 7.);
}

#[test]
fn justifytext_changes_the_justification_without_moving_the_text() {
    let s = Sim::new();
    let id = s.add(Entity::text(p(10., 10.), 2.5, "Hello"));
    let before = text_line_origins(&text_of(&s, id))[0].1;
    s.cmd("JUSTIFYTEXT").click(11., 11.).enter();
    assert!(
        s.prompt()
            .contains("[Left/Align/Fit/Center/Middle/Right/TL/TC/TR/ML/MC/MR/BL/BC/BR] <Left>"),
        "{}",
        s.prompt()
    );
    s.ty("mc");
    let t = text_of(&s, id);
    assert_eq!((t.jh, t.jv), (HJust::Center, VJust::Middle));
    let after = text_line_origins(&t)[0].1;
    assert!(
        (after.x - before.x).abs() < 1e-9 && (after.y - before.y).abs() < 1e-9,
        "{before:?} {after:?}"
    );
    assert!(
        t.p.x > 10. && t.p.y > 10.,
        "the insertion point moved to the centre of the text"
    );
    // unsupported options say so
    s.cmd("JUSTIFYTEXT").click(11., 11.).enter().ty("fit");
    assert!(s.history_has("Align and Fit"), "{}", s.log_text());
    assert!(!s.busy());
}

#[test]
fn mleaderstyle_stores_the_style_in_the_drawing_settings() {
    let s = Sim::new();
    s.cmd("MLEADERSTYLE");
    assert!(s.prompt().contains("<Standard>"), "{}", s.prompt());
    s.ty("Notes");
    assert!(
        s.prompt().contains("Specify text height") && s.prompt().contains("<2.5>"),
        "{}",
        s.prompt()
    );
    s.ty("3.5").ty("2").ty("4");
    assert!(!s.busy());
    let st = s.doc(|d| d.settings.cur_mleader());
    assert_eq!(
        (st.name.as_str(), st.text_height, st.arrow, st.landing),
        ("Notes", 3.5, 2., 4.)
    );
    s.cmd("MLS").ty("Notes");
    assert!(
        s.prompt().contains("<3.5>"),
        "an existing style offers its values: {}",
        s.prompt()
    );
    s.esc();
    // saved with the drawing
    let back = cadforge_core::drawing::Drawing::from_json_str(&s.doc(|d| d.to_json_string()), None)
        .unwrap();
    assert_eq!(back.settings.mleader_styles["Notes"].text_height, 3.5);
    assert_eq!(back.settings.cur_mleader_style, "Notes");
}

#[test]
fn mleaderedit_adds_and_removes_leader_lines() {
    let s = Sim::new();
    let first = s.add(Entity::leader(vec![p(10., 10.), p(20., 20.)], "Note", 2.5));
    s.cmd("MLEADEREDIT").click(15., 15.);
    assert!(
        s.prompt().contains("Specify leader arrowhead location"),
        "{}",
        s.prompt()
    );
    s.click(30., 5.);
    assert!(!s.busy());
    let leaders: Vec<Entity> = s
        .ents()
        .into_iter()
        .filter(|e| matches!(e.kind, Kind::Leader { .. }))
        .collect();
    assert_eq!(leaders.len(), 2);
    assert!(
        leaders[0].grp.is_some() && leaders[0].grp == leaders[1].grp,
        "the leaders form one multileader"
    );
    assert!(
        matches!(&leaders[1].kind, Kind::Leader { pts, s, .. } if pts == &vec![p(30., 5.), p(20., 20.)] && s.is_empty())
    );
    // remove the leader that carries the text: the text moves to the remaining one
    s.cmd("MLEADEREDIT")
        .click(15., 15.)
        .ty("remove")
        .click(15., 15.);
    assert!(!s.busy());
    let left: Vec<Entity> = s
        .ents()
        .into_iter()
        .filter(|e| matches!(e.kind, Kind::Leader { .. }))
        .collect();
    assert_eq!(left.len(), 1);
    assert!(matches!(&left[0].kind, Kind::Leader { s, .. } if s == "Note"));
    assert!(s.doc(|d| d.by_id(first).is_none()));
    // the last leader line cannot be removed
    s.cmd("MLEADEREDIT").click(25., 12.5).ty("remove");
    assert!(
        s.history_has("at least one leader line"),
        "{}",
        s.log_text()
    );
    assert!(!s.busy());
}

#[test]
fn tolerance_draws_a_feature_control_frame() {
    let s = Sim::new();
    s.cmd("TOLERANCE").ty("zzz");
    assert!(
        s.prompt().contains("geometric characteristic symbol") && s.history_has("Unknown symbol"),
        "{}",
        s.prompt()
    );
    s.ty("position")
        .ty("%%c0.5 M")
        .ty("A")
        .ty("B")
        .enter()
        .click(0., 0.);
    assert!(!s.busy());
    let ents = s.ents();
    assert_eq!(
        ents.len(),
        1 + 3 + 4,
        "frame, three dividers, four cell texts"
    );
    let grp = ents[0].grp.clone().unwrap();
    assert!(
        ents.iter().all(|e| e.grp.as_ref() == Some(&grp)),
        "one group"
    );
    let texts: Vec<String> = ents
        .iter()
        .filter_map(|e| {
            if let Kind::Text(t) = &e.kind {
                Some(t.s.clone())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(texts, vec!["\u{2316}", "\u{2300}0.5 M", "A", "B"]);
    // without a symbol and datums: a single tolerance cell
    s.cmd("TOL").enter().ty("0.1").enter().click(0., 20.);
    assert_eq!(s.n(), 8 + 1 + 1);
    assert_eq!(s.doc(|d| d.undo_depth()), 2);
}
