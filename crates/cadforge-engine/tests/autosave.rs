//! Autosave through the command set: the SAVETIME variable and DRAWINGRECOVERY.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::Sim;

#[test]
fn drawingrecovery_reports_the_timer_and_the_recovery_files() {
    let s = Sim::new();
    s.cmd("DRAWINGRECOVERY");
    assert!(
        s.history_has("Autosave is on: unsaved changes are kept every 10 minutes"),
        "{}",
        s.log_text()
    );
    assert!(s.history_has("No open drawing has a recovery file yet."));

    s.add(Entity::line(pt(0., 0.), pt(5., 5.)));
    s.doc_mut(|d| d.dirty = true);
    {
        let mut a = s.h.borrow_mut();
        a.autosave.salt = "t".into();
        a.now = 0.0;
        a.autosave_tick(false);
        a.now = 10.0 * 60_000.0;
        assert_eq!(a.autosave_tick(false).write.len(), 1);
    }
    s.cmd("DRAWINGRECOVERY");
    assert!(
        s.history_has("Drawings with a recovery file") && s.history_has("Drawing1  [t-"),
        "{}",
        s.log_text()
    );
}

#[test]
fn savetime_is_a_setting_and_zero_turns_autosave_off() {
    let s = Sim::new();
    assert_eq!(s.pref("SAVETIME"), 10.0);
    s.setvar("SAVETIME", 0.0);
    s.cmd("DRAWINGRECOVERY");
    assert!(
        s.history_has("Autosave is off (SAVETIME is 0)"),
        "{}",
        s.log_text()
    );
    s.setvar("SAVETIME", 1.0);
    s.cmd("DRAWINGRECOVERY");
    assert!(
        s.history_has("every 1 minute (system variable SAVETIME)"),
        "{}",
        s.log_text()
    );
}

#[test]
fn a_recovered_drawing_keeps_its_geometry_layers_and_custom_definitions() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add_layer("Walls").unwrap();
    });
    s.add(Entity::line(pt(0., 0.), pt(10., 0.)).on_layer("Walls"));
    s.cmd_args(
        "LOADPAT",
        cadforge_engine::runtime::Args::none().with(
            "text",
            cadforge_engine::runtime::ArgVal::Str("*AUTOSAVE_TEST_PAT, p\n0, 0,0, 0,3\n".into()),
        ),
    );
    s.doc_mut(|d| d.dirty = true);
    let file = {
        let mut a = s.h.borrow_mut();
        a.now = 0.0;
        a.autosave_tick(false);
        a.now = 11.0 * 60_000.0;
        a.autosave_tick(false).write.remove(0)
    };
    let fresh = Sim::new();
    let name = fresh
        .h
        .borrow_mut()
        .recover_autosave(&file.key, &file.json)
        .unwrap();
    assert_eq!(name, "Drawing1 (recovered)");
    fresh.doc(|d| {
        assert_eq!(d.entities().len(), 1);
        assert_eq!(d.entities()[0].layer, "Walls");
        assert_eq!(d.custom_patterns.len(), 1);
        assert!(d.dirty);
    });
}
