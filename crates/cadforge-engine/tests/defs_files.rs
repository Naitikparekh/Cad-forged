//! -LINETYPE Load / ? / Set and LOADPAT: definition files reaching the drawing.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::hatch::{hatch_lines, pattern};
use cadforge_core::style::linetype;
use cadforge_engine::app::Event;
use cadforge_engine::runtime::{ArgVal, Args};
use cadforge_engine::sim::Sim;

const LIN: &str = "; test file\n*ENG_FENCE, fence\nA,12,-6\n*ENG_UTIL, utility\nA,8,-2,0,-2\n*ENG_COMPLEX, gas\nA,12,[\"GAS\",STANDARD,S=2.5,X=-2.54],-5\n";
const PAT: &str = "*ENG_WEAVE, weave\n0, 0,1, 0,4\n90, 0,0, 0,4, 2,-2\n*ENG_BAD, bad\n0, 1\n";

fn load(s: &Sim, text: &str, names: &str) {
    s.cmd_args(
        "-LINETYPE",
        Args::none()
            .with("option", ArgVal::Str("Load".into()))
            .with("text", ArgVal::Str(text.into()))
            .with("names", ArgVal::Str(names.into())),
    );
}

#[test]
fn load_brings_linetypes_into_the_drawing_and_reports_what_was_skipped() {
    let s = Sim::new();
    load(&s, LIN, "*");
    assert!(
        s.history_has("Loaded 2 linetypes: ENG_FENCE, ENG_UTIL."),
        "{}",
        s.log_text()
    );
    assert!(
        s.history_has("shapes and text in linetypes are not supported"),
        "{}",
        s.log_text()
    );
    assert_eq!(linetype("eng_fence").unwrap().pattern, &[12.0, -6.0]);
    assert_eq!(s.doc(|d| d.custom_ltypes.len()), 2);
    assert!(s.doc(|d| d.dirty), "loading changes the drawing");
}

#[test]
fn load_can_pick_linetypes_by_name() {
    let s = Sim::new();
    load(&s, LIN, "eng_util");
    assert!(
        s.history_has("Loaded 1 linetype: ENG_UTIL."),
        "{}",
        s.log_text()
    );
    assert_eq!(
        s.doc(|d| d
            .custom_ltypes
            .iter()
            .map(|l| l.name.clone())
            .collect::<Vec<_>>()),
        vec!["ENG_UTIL"]
    );
}

#[test]
fn a_loaded_linetype_can_be_set_current_and_applied() {
    let s = Sim::new();
    load(&s, LIN, "*");
    s.cmd_args(
        "-LINETYPE",
        Args::none().with("option", ArgVal::Str("Set".into())),
    );
    s.ty("eng_fence");
    assert_eq!(
        s.doc(|d| d.cur.ltype.clone()),
        Some("ENG_FENCE".to_string()),
        "{}",
        s.log_text()
    );
    s.cmd("LINE").click(0., 0.).click(10., 0.).enter();
    assert_eq!(s.ent(0).ltype.as_deref(), Some("ENG_FENCE"));
    s.cmd("-LINETYPE").ty("?");
    assert!(
        s.history_has("ENG_FENCE") && s.history_has("DASHED") && s.history_has("fence"),
        "{}",
        s.log_text()
    );
    // an unknown name is refused
    s.cmd("-LINETYPE").ty("Set").ty("nosuch");
    assert!(
        s.history_has("Linetype \"nosuch\" is not loaded."),
        "{}",
        s.log_text()
    );
}

#[test]
fn without_text_the_host_is_asked_for_a_file() {
    let s = Sim::new();
    s.h.borrow_mut().ui.dialogs = true;
    s.h.borrow_mut().take_events();
    s.cmd_args(
        "-LINETYPE",
        Args::none().with("option", ArgVal::Str("Load".into())),
    );
    let asks: Vec<String> =
        s.h.borrow_mut()
            .take_events()
            .into_iter()
            .filter_map(|e| {
                if let Event::UiAction { name, .. } = e {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();
    assert_eq!(asks, vec!["LINETYPELOAD".to_string()]);
    s.cmd("LOADPAT");
    let asks: Vec<String> =
        s.h.borrow_mut()
            .take_events()
            .into_iter()
            .filter_map(|e| {
                if let Event::UiAction { name, .. } = e {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();
    assert_eq!(asks, vec!["LOADPAT".to_string()]);
    // headless there is no chooser
    let h = Sim::new();
    h.cmd("LOADPAT");
    assert!(h.history_has("needs a file chooser"), "{}", h.log_text());
}

#[test]
fn loadpat_makes_patterns_usable_by_name() {
    let s = Sim::new();
    s.cmd_args(
        "LOADPAT",
        Args::none().with("text", ArgVal::Str(PAT.into())),
    );
    assert!(
        s.history_has("Loaded 1 hatch pattern: ENG_WEAVE."),
        "{}",
        s.log_text()
    );
    assert!(
        s.history_has("ENG_BAD: a line family needs angle"),
        "{}",
        s.log_text()
    );
    assert!(pattern("eng_weave").is_some());
    // the hatch tool uses whatever pattern is current: pick the loaded one (the gallery lists it) and hatch a square
    assert!(cadforge_core::hatch::all_patterns()
        .iter()
        .any(|p| p.name == "ENG_WEAVE"));
    s.h.borrow().mem.hatch.borrow_mut().pattern = "ENG_WEAVE".into();
    s.add(Entity::pline_pts(
        &[pt(0., 0.), pt(20., 0.), pt(20., 20.), pt(0., 20.)],
        true,
    ));
    s.cmd("HATCH").click(10., 10.).enter();
    let h = s
        .ents()
        .into_iter()
        .find_map(|e| {
            if let Kind::Hatch(h) = e.kind {
                Some(h)
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("no hatch: {}", s.log_text()));
    assert_eq!(h.pattern, "ENG_WEAVE");
    let lines = hatch_lines(
        &[vec![pt(0., 0.), pt(20., 0.), pt(20., 20.), pt(0., 20.)]],
        &h.pattern,
        1.0,
        0.0,
    )
    .unwrap();
    assert!(lines.len() > 8, "a grid of lines: {}", lines.len());
}
