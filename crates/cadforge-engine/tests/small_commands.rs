//! MTEXTEDIT, TEXTALIGN, TCOUNT, PASTESPEC, QTEXT, DIMROTATED, SECTION and LAYERPMODE.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::dim::DimData;
use cadforge_engine::sim::Sim;

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn texts(s: &Sim) -> Vec<(String, Pt)> {
    s.ents().into_iter().filter_map(|e| if let Kind::Text(t) = e.kind { Some((t.s, t.p)) } else { None }).collect()
}

#[test]
fn textalign_moves_text_onto_a_horizontal_or_slanted_line() {
    let s = Sim::new();
    for (x, y) in [(0., 3.), (10., -2.), (20., 8.)] {
        s.add(Entity::text(p(x, y), 2.5, "T"));
    }
    s.cmd("TEXTALIGN").click(0., 3.).click(10., -2.).enter();
    // that window selected two of them (the first two corners); select all by crossing instead
    let s = Sim::new();
    let ids: Vec<u64> = [(0., 3.), (10., -2.), (20., 8.)].iter().map(|(x, y)| s.add(Entity::text(p(*x, *y), 2.5, "T"))).collect();
    s.set_sel(&ids);
    s.cmd("TEXTALIGN").click(-5., 0.).enter();          // a point on the line, then Enter = horizontal
    let ys: Vec<f64> = texts(&s).iter().map(|t| t.1.y).collect();
    assert!(ys.iter().all(|y| y.abs() < 1e-9), "{ys:?}\n{}", s.log_text());
    assert_eq!(texts(&s).iter().map(|t| t.1.x).collect::<Vec<_>>(), vec![0.0, 10.0, 20.0], "only moved across the line");
    assert!(s.history_has("2 text objects aligned.") || s.history_has("3 text objects aligned."), "{}", s.log_text());

    // a 45 degree line through the origin: y = x
    let t = Sim::new();
    let id = t.add(Entity::text(p(10., 0.), 2.5, "T"));
    t.set_sel(&[id]);
    t.cmd("TEXTALIGN").click(0., 0.).click(5., 5.);
    let q = texts(&t)[0].1;
    assert!((q.x - 5.0).abs() < 1e-9 && (q.y - 5.0).abs() < 1e-9, "{q:?}");
}

#[test]
fn tcount_numbers_text_in_selection_order() {
    let s = Sim::new();
    let ids: Vec<u64> = ["a", "b", "c"].iter().enumerate().map(|(i, t)| s.add(Entity::text(p(i as f64 * 10., 0.), 2.5, t))).collect();
    s.set_sel(&ids);
    s.cmd("TCOUNT").ty("5").ty("5").enter();
    assert_eq!(texts(&s).into_iter().map(|t| t.0).collect::<Vec<_>>(), vec!["5", "10", "15"], "{}", s.log_text());
    s.set_sel(&ids);
    s.cmd("TCOUNT").ty("1").ty("1").ty("Prefix");
    assert_eq!(texts(&s).into_iter().map(|t| t.0).collect::<Vec<_>>(), vec!["15", "210", "315"]);
    s.set_sel(&ids);
    s.cmd("TCOUNT").ty("0.5").ty("0.25").ty("Suffix");
    assert_eq!(texts(&s)[2].0, "3151");
    assert_eq!(texts(&s)[0].0, "150.5");
}

#[test]
fn mtextedit_edits_multiline_text_only() {
    let s = Sim::new();
    let one = s.add(Entity::text(p(0., 0.), 2.5, "plain"));
    let mut mt = Entity::text(p(0., 20.), 2.5, "multi");
    if let Kind::Text(t) = &mut mt.kind {
        t.multi = true;
    }
    let multi = s.add(mt);
    // a single-line text cannot be picked by MTEXTEDIT
    s.cmd("MTEXTEDIT").click(0.5, 0.5);
    assert!(s.busy(), "nothing valid picked: {}", s.log_text());
    s.esc();
    s.cmd_args("MTEXTEDIT", cadforge_engine::runtime::Args::none().with("entity", cadforge_engine::runtime::ArgVal::Id(multi)));
    assert_eq!(s.running().as_deref(), Some("TEXTEDIT"), "it hands over to the editor: {}", s.log_text());
    s.ty("rewritten");
    let new: Vec<String> = texts(&s).into_iter().map(|t| t.0).collect();
    assert_eq!(new, vec!["plain", "rewritten"], "{}", s.log_text());
    let _ = one;
}

#[test]
fn pastespec_pastes_the_clipboard_the_way_you_choose() {
    let s = Sim::new();
    let id = s.add(Entity::circle(p(5., 5.), 2.));
    s.set_sel(&[id]);
    s.cmd("COPYBASE").click(5., 5.);
    s.cmd("PASTESPEC").ty("Original coordinates");
    assert_eq!(s.n(), 2, "{}", s.log_text());
    let Kind::Circle { c, .. } = s.ent(1).kind else { panic!() };
    assert_eq!((c.x, c.y), (5., 5.), "pasted where it was copied from");
    s.cmd("PASTESPEC").ty("Block").click(30., 30.).enter();
    assert!(s.ents().iter().any(|e| matches!(e.kind, Kind::Insert(_))), "{}", s.log_text());
}

#[test]
fn qtext_switches_the_variable_the_renderer_reads() {
    let s = Sim::new();
    assert_eq!(s.pref("QTEXTMODE"), 0.0);
    s.cmd("QTEXT").ty("ON");
    assert_eq!(s.pref("QTEXTMODE"), 1.0);
    assert!(s.history_has("Quick text is on"));
    s.cmd("QTEXT").ty("OFF");
    assert_eq!(s.pref("QTEXTMODE"), 0.0);
}

#[test]
fn dimrotated_asks_for_the_dimension_line_angle() {
    let s = Sim::new();
    s.cmd("DIMROTATED").click(0., 0.).click(10., 10.).ty("30").click(5., 20.);
    let dims: Vec<DimData> = s.ents().into_iter().filter_map(|e| if let Kind::Dim(d) = e.kind { Some(d) } else { None }).collect();
    assert_eq!(dims.len(), 1, "{}", s.log_text());
    assert!((dims[0].ang.unwrap() - 30f64.to_radians()).abs() < 1e-9, "{:?}", dims[0].ang);
}

#[test]
fn layerpmode_off_stops_layerp_from_remembering_changes() {
    let on = Sim::new();
    on.doc_mut(|d| {
        d.add_layer("A").unwrap();
    });
    on.cmd("LAYERP");   // let the tracker see the drawing
    on.doc_mut(|d| d.change(|d| d.layer_mut("A").unwrap().on = false));
    on.cmd("LAYERP");
    assert!(on.doc(|d| d.layer("A").unwrap().on), "LAYERP restored the layer: {}", on.log_text());

    let off = Sim::new();
    off.doc_mut(|d| {
        d.add_layer("A").unwrap();
    });
    off.cmd("LAYERPMODE").ty("OFF");
    assert_eq!(off.pref("LAYERPMODE"), 0.0);
    off.cmd("LAYERP");
    off.doc_mut(|d| d.change(|d| d.layer_mut("A").unwrap().on = false));
    off.cmd("LAYERP");
    assert!(!off.doc(|d| d.layer("A").unwrap().on), "nothing to go back to: {}", off.log_text());
    assert!(off.history_has("There are no earlier layer settings") || off.history_has("No previous"), "{}", off.log_text());
}

#[test]
fn section_makes_a_region_where_the_plane_cuts_the_solid() {
    let s = Sim::new();
    s.play("BOX", &[cadforge_engine::sim::Step::Click(0., 0.), cadforge_engine::sim::Step::Click(10., 10.), cadforge_engine::sim::Step::Type("10")]);
    assert_eq!(s.solids().len(), 1, "{}", s.log_text());
    let id = s.solids()[0].id;
    s.set_sel(&[id]);
    s.cmd("SECTION").ty("XY").click(0., 0.).ty("5");
    let regions: Vec<RegionData> = s.ents().into_iter().filter_map(|e| if let Kind::Region(r) = e.kind { Some(r) } else { None }).collect();
    assert_eq!(regions.len(), 1, "{}", s.log_text());
    let b = Bounds::of_points(&regions[0].loops[0].iter().map(|v| v.pt()).collect::<Vec<_>>());
    assert!((b.max_x - b.min_x - 10.0).abs() < 1e-6 && (b.max_y - b.min_y - 10.0).abs() < 1e-6, "{b:?}");
    assert_eq!(s.ents().last().unwrap().layer, "SECTION");
}
