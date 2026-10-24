//! Isometric drafting (ISODRAFT, ISOPLANE, SNAPSTYL), additional polar angles (POLARADDANG) and the .bak copy kept
//! when a drawing is saved over a file (ISAVEBAK).
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::autosave::{backup_path, write_keeping_backup};
use cadforge_engine::sim::Sim;

fn line_ends(s: &Sim, i: usize) -> (Pt, Pt) {
    let Kind::Line { a, b } = s.ent(i).kind else { panic!("a line: {:?}", s.ent(i).kind) };
    (a, b)
}

fn cursor(s: &Sim) -> Pt {
    s.h.borrow().rt.cursor.pt
}

#[test]
fn isodraft_and_isoplane_set_the_isometric_snap_and_plane() {
    let s = Sim::new();
    s.cmd("ISODRAFT").ty("isoTop");
    assert_eq!((s.pref("SNAPSTYL"), s.pref("ISOPLANE")), (1.0, 1.0), "{}", s.log_text());
    assert!(s.history_has("Current isoplane: Top"), "{}", s.log_text());
    // Enter moves on to the next plane, like F5
    s.cmd("ISOPLANE").ty("");
    assert_eq!(s.pref("ISOPLANE"), 2.0, "{}", s.log_text());
    s.cmd("ISOPLANE").ty("Left");
    assert_eq!(s.pref("ISOPLANE"), 0.0);
    s.cmd("ISODRAFT").ty("Orthographic");
    assert_eq!(s.pref("SNAPSTYL"), 0.0, "{}", s.log_text());
}

#[test]
fn isometric_snap_puts_points_on_the_isometric_lattice() {
    let s = Sim::new();
    s.cmd("ISODRAFT").ty("isoTop");
    s.setvar("SNAPMODE", 1.0);
    s.setvar("OSMODE", 16384.0);
    s.cmd("LINE").click(8.0, 4.0).click(-9.0, 5.5).enter();
    let (a, b) = line_ends(&s, 0);
    let c30 = 3f64.sqrt() / 2.0;
    assert!(dist(a, pt(10.0 * c30, 5.0)) < 1e-9 && dist(b, pt(-10.0 * c30, 5.0)) < 1e-9, "{a:?} {b:?}");
}

#[test]
fn ortho_follows_the_axes_of_the_current_isoplane() {
    let s = Sim::new();
    s.setvar("OSMODE", 16384.0);
    s.setvar("ORTHOMODE", 1.0);
    s.cmd("ISODRAFT").ty("isoRight");
    s.cmd("LINE").ty("0,0");
    // the right plane has the 30° and the vertical axes; the cursor is closest to 30°
    s.mv(40.0, 20.0);
    let p = cursor(&s);
    assert!((p.y.atan2(p.x).to_degrees() - 30.0).abs() < 1e-9, "{p:?}");
    s.mv(3.0, 40.0);
    assert!(cursor(&s).x.abs() < 1e-9, "the vertical axis: {:?}", cursor(&s));
    s.esc();
    s.cmd("ISOPLANE").ty("Top");
    s.cmd("LINE").ty("0,0");
    s.mv(-40.0, 20.0);
    let p = cursor(&s);
    assert!((p.y.atan2(p.x).to_degrees() - 150.0).abs() < 1e-9, "top plane: 30° and 150°: {p:?}");
    s.esc();
}

#[test]
fn polar_tracking_also_follows_the_additional_angles() {
    let s = Sim::new();
    s.setvar("OSMODE", 16384.0);
    s.setvar("OTRACKMODE", 0.0);
    s.setvar("POLARMODE", 1.0);
    let raw = pt(100.0 * 17f64.to_radians().cos(), 100.0 * 17f64.to_radians().sin() + 0.1);
    s.cmd("LINE").ty("0,0");
    s.mv(raw.x, raw.y);
    assert_eq!(cursor(&s), raw, "17° is not a multiple of POLARANG");
    s.esc();
    let _ = s.h.borrow_mut().setvar("POLARADDANG", "17;35.5");
    s.cmd("LINE").ty("0,0");
    s.mv(raw.x, raw.y);
    let p = cursor(&s);
    assert!((p.y.atan2(p.x).to_degrees() - 17.0).abs() < 1e-9, "{p:?}");
    assert!(s.h.borrow().rt.cursor.track.as_ref().is_some_and(|t| t.label.ends_with("< 17°")));
    s.esc();
}

#[test]
fn saving_over_a_file_keeps_the_old_version_as_a_bak_file() {
    let dir = std::env::temp_dir().join(format!("cadforge-bak-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("plan.dwg");
    assert_eq!(backup_path(&f), dir.join("plan.bak"));
    // a new file has nothing to back up
    assert_eq!(write_keeping_backup(&f, b"first", true).unwrap(), None);
    assert_eq!(write_keeping_backup(&f, b"second", true).unwrap(), Some(dir.join("plan.bak")));
    assert_eq!(std::fs::read(dir.join("plan.bak")).unwrap(), b"first");
    assert_eq!(std::fs::read(&f).unwrap(), b"second");
    // ISAVEBAK 0: the backup is left alone
    assert_eq!(write_keeping_backup(&f, b"third", false).unwrap(), None);
    assert_eq!(std::fs::read(dir.join("plan.bak")).unwrap(), b"first");
    assert_eq!(Sim::new().pref("ISAVEBAK"), 1.0, "backups are on by default, as in AutoCAD");
    let _ = std::fs::remove_dir_all(&dir);
}
