//! ISOLATEOBJECTS / HIDEOBJECTS / UNISOLATEOBJECTS.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::Sim;

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn three_lines(s: &Sim) -> [u64; 3] {
    [s.add(Entity::line(p(0., 0.), p(10., 0.))), s.add(Entity::line(p(0., 5.), p(10., 5.))), s.add(Entity::line(p(0., 10.), p(10., 10.)))]
}

fn visible(s: &Sim, id: u64) -> bool {
    s.doc(|d| d.is_visible(d.by_id(id).unwrap()))
}

#[test]
fn isolate_hides_everything_but_the_selection_and_unisolate_brings_it_back() {
    let s = Sim::new();
    let [a, b, c] = three_lines(&s);
    s.set_sel(&[b]);
    s.cmd("ISOLATEOBJECTS");
    assert!(!visible(&s, a) && visible(&s, b) && !visible(&s, c), "{}", s.log_text());
    assert!(s.history_has("2 objects hidden."));
    // hidden objects cannot be picked
    s.cmd("ERASE").click(5., 0.).enter();
    assert_eq!(s.n(), 3, "the hidden line was not erased: {}", s.log_text());
    s.cmd("UNISOLATEOBJECTS");
    assert!(visible(&s, a) && visible(&s, b) && visible(&s, c));
    assert!(s.history_has("2 objects displayed."));
}

#[test]
fn hideobjects_hides_only_the_selected_objects_and_is_not_saved() {
    let s = Sim::new();
    let [a, b, _] = three_lines(&s);
    s.set_sel(&[a, b]);
    s.cmd("HIDEOBJECTS");
    assert!(!visible(&s, a) && !visible(&s, b));
    // the hidden state is session-only: it is not in the saved file and not part of undo
    let json = s.doc(|d| d.to_json_string());
    assert!(!json.contains("hidden_objs"), "not persisted");
    s.cmd("UNISOLATEOBJECTS");
    assert!(visible(&s, a));
    s.cmd("UNISOLATEOBJECTS");
    assert!(s.history_has("No objects are hidden."));
}
