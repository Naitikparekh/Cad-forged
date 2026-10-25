//! UNDO options: Mark and Back, BEgin and End groups, Control (All / One / None).
use cadforge_engine::sim::Sim;

fn line(s: &Sim, y: f64) {
    s.cmd("LINE").ty(&format!("0,{y}")).ty(&format!("10,{y}")).enter();
}

#[test]
fn back_undoes_everything_since_the_mark() {
    let s = Sim::new();
    line(&s, 0.);
    s.cmd("UNDO").ty("Mark");
    line(&s, 5.);
    line(&s, 10.);
    assert_eq!(s.n(), 3);
    s.cmd("UNDO").ty("Back");
    assert_eq!(s.n(), 1, "{}", s.log_text());
    assert!(s.history_has("Mark encountered: 2 commands undone"));
    // without a mark Back undoes everything
    line(&s, 5.);
    s.cmd("UNDO").ty("Back");
    assert_eq!(s.n(), 0, "{}", s.log_text());
    assert!(s.history_has("Everything has been undone"));
}

#[test]
fn a_group_is_one_undo_step() {
    let s = Sim::new();
    line(&s, 0.);
    s.cmd("UNDO").ty("BEgin");
    line(&s, 5.);
    line(&s, 10.);
    line(&s, 15.);
    s.cmd("UNDO").ty("BEgin");
    assert!(s.history_has("An undo group is already open"), "{}", s.log_text());
    s.cmd("UNDO").ty("End");
    assert!(s.history_has("Undo group ended (3 commands)."), "{}", s.log_text());
    s.cmd("U");
    assert_eq!(s.n(), 1, "the three lines of the group go together: {}", s.log_text());
    s.cmd("REDO");
    assert_eq!(s.n(), 4);
    s.cmd("UNDO").ty("End");
    assert!(s.history_has("No undo group is open."));
}

#[test]
fn control_one_and_none_limit_what_can_be_undone() {
    let s = Sim::new();
    line(&s, 0.);
    line(&s, 5.);
    s.cmd("UNDO").ty("Control").ty("One");
    line(&s, 10.);
    s.cmd("U").cmd("U");
    assert_eq!(s.n(), 2, "only the last step is kept: {}", s.log_text());
    s.cmd("UNDO").ty("Control").ty("None");
    line(&s, 15.);
    s.cmd("U");
    assert_eq!(s.n(), 3, "nothing is recorded: {}", s.log_text());
    s.cmd("UNDO").ty("Control").ty("All");
    line(&s, 20.);
    line(&s, 25.);
    s.cmd("UNDO").ty("2");
    assert_eq!(s.n(), 3);
}
