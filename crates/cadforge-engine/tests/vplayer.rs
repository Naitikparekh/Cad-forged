//! VPLAYER, LAYVPI and LAYERPALL: layer settings that belong to one layout viewport.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::Sim;

fn viewports(s: &Sim) -> Vec<(u64, ViewportData)> {
    s.doc(|d| {
        d.layout("Layout1")
            .unwrap()
            .entities
            .iter()
            .filter_map(|e| {
                if let Kind::Viewport(v) = &e.kind {
                    Some((e.id, v.clone()))
                } else {
                    None
                }
            })
            .collect()
    })
}

/// A drawing with layers A, B and C in model space (one line each) and Layout1 showing a viewport.
fn drawing() -> Sim {
    let s = Sim::new();
    s.doc_mut(|d| {
        for n in ["A", "B", "C"] {
            d.add_layer(n).unwrap();
        }
    });
    for (i, n) in ["A", "B", "C"].iter().enumerate() {
        s.add(Entity::line(pt(0., i as f64 * 10.), pt(50., i as f64 * 10.)).on_layer(n));
    }
    s.cmd("LAYOUT").ty("Set").ty("Layout1");
    s
}

fn frozen_in(s: &Sim, layer: &str) -> Vec<bool> {
    viewports(s)
        .iter()
        .map(|(_, v)| v.is_frozen(layer))
        .collect()
}

#[test]
fn freeze_thaw_and_reset_in_all_viewports() {
    let s = drawing();
    s.cmd("VPLAYER").ty("Freeze").ty("A,B").ty("All");
    assert_eq!(frozen_in(&s, "A"), vec![true], "{}", s.log_text());
    assert_eq!(frozen_in(&s, "B"), vec![true]);
    assert_eq!(frozen_in(&s, "C"), vec![false]);
    assert!(
        s.doc(|d| !d.layer("A").unwrap().frozen),
        "the layer itself is not frozen: only its viewport state"
    );
    assert!(
        s.history_has("2 layers in 1 viewport: frozen."),
        "{}",
        s.log_text()
    );
    s.cmd("VPLAYER").ty("Thaw").ty("a").ty("All");
    assert_eq!(frozen_in(&s, "A"), vec![false]);
    s.cmd("VPLAYER").ty("Reset").ty("B").ty("All");
    assert_eq!(frozen_in(&s, "B"), vec![false]);
    assert!(
        viewports(&s)[0].1.layers.is_empty(),
        "nothing left to write"
    );
}

#[test]
fn wildcards_pick_several_layers() {
    let s = drawing();
    s.cmd("VPLAYER").ty("Freeze").ty("*").ty("All");
    assert!(
        ["A", "B", "C", "0"]
            .iter()
            .all(|l| frozen_in(&s, l) == vec![true]),
        "{}",
        s.log_text()
    );
}

#[test]
fn only_the_selected_viewport_changes_and_the_change_can_be_undone() {
    let s = drawing();
    // add a second viewport next to the first
    s.cmd("MVIEW").click(20.0, 20.0).click(60.0, 60.0);
    assert_eq!(viewports(&s).len(), 2);
    let first = viewports(&s)[0].0;
    let pick = s.doc(|d| {
        match &d
            .layout("Layout1")
            .unwrap()
            .entities
            .iter()
            .find(|e| e.id == first)
            .unwrap()
            .kind
        {
            Kind::Viewport(v) => v.outline()[0],
            _ => unreachable!(),
        }
    });
    s.cmd("VPLAYER")
        .ty("Freeze")
        .ty("A")
        .ty("Select")
        .click(pick.x, pick.y)
        .enter();
    assert_eq!(
        frozen_in(&s, "A").iter().filter(|f| **f).count(),
        1,
        "{}",
        s.log_text()
    );
    s.undo();
    assert_eq!(
        frozen_in(&s, "A").iter().filter(|f| **f).count(),
        0,
        "undo restores it"
    );
}

#[test]
fn newfrz_creates_layers_frozen_in_every_viewport_including_later_ones() {
    let s = drawing();
    s.cmd("VPLAYER").ty("Newfrz").ty("Notes");
    assert!(s.doc(|d| d.layer("Notes").is_some()));
    assert_eq!(frozen_in(&s, "Notes"), vec![true]);
    s.cmd("MVIEW").click(20.0, 20.0).click(60.0, 60.0);
    assert_eq!(
        frozen_in(&s, "Notes"),
        vec![true, true],
        "a new viewport starts with Notes frozen"
    );
}

#[test]
fn vpvisdflt_sets_the_default_for_new_viewports_and_optionally_the_current_ones() {
    let s = drawing();
    s.cmd("VPLAYER")
        .ty("Vpvisdflt")
        .ty("B")
        .ty("Frozen")
        .ty("No");
    assert_eq!(
        frozen_in(&s, "B"),
        vec![false],
        "existing viewport left alone"
    );
    s.cmd("MVIEW").click(20.0, 20.0).click(60.0, 60.0);
    assert_eq!(frozen_in(&s, "B"), vec![false, true]);
    s.cmd("VPLAYER")
        .ty("Vpvisdflt")
        .ty("B")
        .ty("Thawed")
        .ty("Yes");
    assert_eq!(frozen_in(&s, "B"), vec![false, false]);
}

#[test]
fn the_question_mark_lists_what_a_viewport_overrides() {
    let s = drawing();
    s.cmd("VPLAYER").ty("Freeze").ty("A").ty("All");
    s.cmd("VPLAYER").ty("Color").ty("B").ty("255,0,0").ty("All");
    let id = viewports(&s)[0].0;
    let pick = s.doc(|d| {
        match &d
            .layout("Layout1")
            .unwrap()
            .entities
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .kind
        {
            Kind::Viewport(v) => v.outline()[0],
            _ => unreachable!(),
        }
    });
    s.cmd("VPLAYER").ty("?").click(pick.x, pick.y).enter();
    assert!(
        s.history_has(&format!("Viewport {id}: A: frozen")),
        "{}",
        s.log_text()
    );
    assert!(
        s.history_has(&format!("Viewport {id}: B: colour 255,0,0")),
        "{}",
        s.log_text()
    );
}

#[test]
fn colour_linetype_lineweight_and_transparency_overrides_are_validated() {
    let s = drawing();
    s.cmd("VPLAYER").ty("Color").ty("A").ty("red-ish");
    assert!(s.history_has("Enter a colour number"), "{}", s.log_text());
    s.cmd("VPLAYER").ty("Linetype").ty("A").ty("NOSUCH");
    assert!(s.history_has("Linetype \"NOSUCH\" is not loaded."));
    s.cmd("VPLAYER").ty("LWeight").ty("A").ty("0.5").ty("All");
    s.cmd("VPLAYER").ty("Transparency").ty("A").ty("95");
    assert!(s.history_has("from 0 to 90"));
    s.cmd("VPLAYER")
        .ty("Transparency")
        .ty("A")
        .ty("40")
        .ty("All");
    s.cmd("VPLAYER")
        .ty("Linetype")
        .ty("A")
        .ty("DASHED")
        .ty("All");
    let o = viewports(&s)[0].1.layer_override("A").cloned().unwrap();
    assert_eq!(
        (o.lw, o.transp, o.ltype.as_deref()),
        (Some(50), Some(40.0), Some("DASHED"))
    );
    s.cmd("VPLAYER")
        .ty("Transparency")
        .ty("A")
        .ty("reset")
        .ty("All");
    assert_eq!(viewports(&s)[0].1.layer_override("A").unwrap().transp, None);
}

#[test]
fn layerpall_removes_property_overrides_but_keeps_vp_freeze() {
    let s = drawing();
    s.cmd("VPLAYER").ty("Freeze").ty("A").ty("All");
    s.cmd("VPLAYER").ty("Color").ty("B").ty("3").ty("All");
    s.cmd("LAYERPALL");
    assert!(
        s.history_has("1 layer override reset in 1 viewport."),
        "{}",
        s.log_text()
    );
    let v = &viewports(&s)[0].1;
    assert!(v.is_frozen("A"), "freeze is VPLAYER Reset's business");
    assert!(v.layer_override("B").is_none());
}

#[test]
fn inside_an_active_viewport_its_frozen_layers_cannot_be_picked() {
    let s = drawing();
    s.cmd("VPLAYER").ty("Freeze").ty("B").ty("All");
    s.cmd("MSPACE");
    assert!(s.doc(|d| d.is_model()), "{}", s.log_text());
    let frozen = s.doc(|d| d.active_vp_frozen.iter().cloned().collect::<Vec<_>>());
    assert_eq!(frozen, vec!["b".to_string()]);
    let visible: Vec<String> = s.doc(|d| {
        d.entities()
            .iter()
            .filter(|e| d.is_visible(e))
            .map(|e| e.layer.clone())
            .collect()
    });
    assert_eq!(visible.len(), 2, "{visible:?}");
    assert!(!visible.contains(&"B".to_string()));
    // a click on the frozen layer's line selects nothing; on a visible layer's line it does
    s.cmd("ERASE").click(25.0, 10.0); // B's line
    assert!(
        s.busy(),
        "ERASE is still waiting for an object: the frozen line was not picked"
    );
    s.esc();
    s.cmd("ERASE").click(25.0, 0.0).enter(); // A's line
    assert_eq!(
        s.doc(|d| d.entities().len()),
        2,
        "A's line was erased, B's and C's remain"
    );
    s.undo();
    // thawing while inside updates what is visible straight away
    s.cmd("VPLAYER").ty("Thaw").ty("B").ty("Current");
    assert!(s.doc(|d| d.active_vp_frozen.is_empty()));
    // and leaving the viewport clears the session state
    s.cmd("VPLAYER").ty("Freeze").ty("C").ty("Current");
    assert!(s.doc(|d| !d.active_vp_frozen.is_empty()));
    s.cmd("PSPACE");
    assert!(s.doc(|d| d.active_vp_frozen.is_empty() && d.active_vp.is_none()));
}

#[test]
fn undo_inside_an_active_viewport_keeps_the_visible_set_in_step() {
    let s = drawing();
    s.cmd("MSPACE");
    s.cmd("VPLAYER").ty("Freeze").ty("A").ty("Current");
    assert!(s.doc(|d| d.active_vp_frozen.contains("a")));
    s.undo();
    assert!(
        s.doc(|d| d.active_vp_frozen.is_empty()),
        "undo thaws it again"
    );
    s.redo();
    assert!(s.doc(|d| d.active_vp_frozen.contains("a")));
}

#[test]
fn layvpi_freezes_everything_but_the_selected_layers_in_this_viewport_only() {
    let s = drawing();
    s.cmd("LAYVPI");
    assert!(
        s.history_has("Activate a viewport first"),
        "{}",
        s.log_text()
    );
    s.cmd("MSPACE");
    s.cmd("LAYVPI").click(25.0, 10.0).enter(); // the line on layer B
    let v = viewports(&s)[0].1.clone();
    assert!(
        v.is_frozen("A") && v.is_frozen("C") && !v.is_frozen("B"),
        "{:?}\n{}",
        v.layers,
        s.log_text()
    );
    assert!(!v.is_frozen("0"), "the current layer is never frozen");
    assert!(
        s.doc(|d| d.layers.iter().all(|l| !l.frozen)),
        "the drawing's own layer states are untouched"
    );
}

#[test]
fn outside_a_layout_vplayer_says_so() {
    let s = Sim::new();
    s.cmd("VPLAYER").ty("Freeze").ty("0");
    assert!(
        s.history_has("switch to a layout first"),
        "{}",
        s.log_text()
    );
}
