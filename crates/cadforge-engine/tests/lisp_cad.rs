//! AutoLISP talking to the drawing: `command`, entities, selection sets, interactive `get...` functions and `c:` commands.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::Sim;

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

/// Type `src` and return the history lines it produced (without the echoed command).
fn ev(s: &Sim, src: &str) -> String {
    let before = s.h.borrow().history.len();
    s.ty(src);
    let h = s.h.borrow();
    h.history.iter().skip(before).map(|l| l.text.clone()).filter(|t| !t.starts_with("Command:")).collect::<Vec<_>>().join("\n")
}

#[test]
fn command_feeds_inputs_to_the_command() {
    let s = Sim::new();
    ev(&s, "(command \"line\" '(0 0) '(10 0) \"\")");
    assert!(!s.busy(), "{}", s.prompt());
    assert_eq!(s.n(), 1);
    let Kind::Line { a, b } = s.ent(0).kind else { panic!() };
    assert_eq!((a, b), (p(0., 0.), p(10., 0.)));
    ev(&s, "(command \"_.circle\" '(5 5) 3)");
    let Kind::Circle { c, r } = s.ent(1).kind else { panic!() };
    assert_eq!((c, r), (p(5., 5.), 3.0));
    // several commands in one call; strings are typed as they are
    ev(&s, "(command \"rectang\" \"20,20\" \"30,25\" \"circle\" \"40,40\" \"d\" \"8\")");
    assert_eq!(s.n(), 4);
    let Kind::Circle { r, .. } = s.ent(3).kind else { panic!() };
    near(r, 4.0);
}

#[test]
fn command_passes_entities_and_selection_sets() {
    let s = Sim::new();
    ev(&s, "(command \"circle\" '(0 0) 2)");
    ev(&s, "(command \"circle\" '(20 0) 2)");
    ev(&s, "(setq e (entlast))");
    ev(&s, "(command \"move\" e \"\" '(0 0) '(0 10))");
    let Kind::Circle { c, .. } = s.ent(1).kind else { panic!() };
    assert_eq!(c, p(20., 10.));
    ev(&s, "(setq ss (ssget \"X\"))");
    assert_eq!(ev(&s, "(sslength ss)"), "2");
    ev(&s, "(command \"erase\" ss \"\")");
    assert_eq!(s.n(), 0);
}

#[test]
fn pause_leaves_one_answer_to_the_user() {
    let s = Sim::new();
    ev(&s, "(command \"circle\" '(0 0) PAUSE)");
    assert!(s.busy(), "waiting for the radius");
    assert!(s.prompt().contains("radius") || s.prompt().contains("Diameter"), "{}", s.prompt());
    s.ty("7");
    assert!(!s.busy());
    let Kind::Circle { r, .. } = s.ent(0).kind else { panic!() };
    near(r, 7.0);
    // and a point picked with the mouse
    ev(&s, "(command \"line\" '(0 0) PAUSE \"\")");
    s.click(30., 40.);
    assert!(!s.busy());
    let Kind::Line { b, .. } = s.ent(1).kind else { panic!() };
    assert_eq!(b, p(30., 40.));
}

#[test]
fn entget_describes_entities_like_dxf() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add_layer("Walls").unwrap();
    });
    ev(&s, "(command \"circle\" '(5 6) 3)");
    ev(&s, "(setq e (entlast))");
    ev(&s, "(setq g (entget e))");
    assert_eq!(ev(&s, "(cdr (assoc 0 g))"), "\"CIRCLE\"");
    assert_eq!(ev(&s, "(cdr (assoc 8 g))"), "\"0\"");
    assert_eq!(ev(&s, "(cdr (assoc 40 g))"), "3.0");
    assert_eq!(ev(&s, "(cdr (assoc 10 g))"), "(5.0 6.0 0.0)");
    assert_eq!(ev(&s, "(= e (cdr (assoc -1 g)))"), "T");
    // entmod changes the object
    ev(&s, "(setq g (subst (cons 40 9.5) (assoc 40 g) g))");
    ev(&s, "(setq g (subst (cons 8 \"Walls\") (assoc 8 g) g))");
    ev(&s, "(entmod g)");
    let e = s.ent(0);
    assert_eq!(e.layer, "Walls");
    let Kind::Circle { r, .. } = e.kind else { panic!() };
    near(r, 9.5);
    // a layer that does not exist is refused
    assert_eq!(ev(&s, "(entmod (subst (cons 8 \"Nope\") (assoc 8 (entget e)) (entget e)))"), "nil");
}

#[test]
fn entmake_creates_objects_and_validates() {
    let s = Sim::new();
    assert_eq!(ev(&s, "(entmake (list '(0 . \"LINE\") '(10 0.0 0.0 0.0) '(11 10.0 5.0 0.0)))").starts_with("((0 . \"LINE\")"), true);
    ev(&s, "(entmake '((0 . \"CIRCLE\") (10 3.0 3.0 0.0) (40 . 2.0) (62 . 1)))");
    ev(&s, "(entmake (list '(0 . \"LWPOLYLINE\") '(90 . 3) '(70 . 1) '(10 0.0 0.0) '(10 4.0 0.0) '(42 . 0.5) '(10 4.0 4.0)))");
    ev(&s, "(entmake '((0 . \"TEXT\") (1 . \"HELLO\") (10 1.0 1.0 0.0) (40 . 2.5)))");
    assert_eq!(s.n(), 4);
    assert_eq!(s.ent(1).color, Some(cadforge_core::style::Color::Aci(1)));
    let Kind::Pline { v, closed } = s.ent(2).kind else { panic!() };
    assert!(closed);
    assert_eq!(v.len(), 3);
    near(v[1].b, 0.5);
    assert!(matches!(&s.ent(3).kind, Kind::Text(t) if t.s == "HELLO"));
    assert_eq!(ev(&s, "(entmake '((0 . \"CIRCLE\") (10 3.0 3.0 0.0)))"), "nil", "a circle needs a radius");
    assert_eq!(ev(&s, "(entmake '((0 . \"NOPE\")))"), "nil");
}

#[test]
fn ssget_filters_and_selection_set_functions() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add_layer("WALLS").unwrap();
        d.add_layer("DOORS").unwrap();
        d.add(Entity::line(p(0., 0.), p(10., 0.)).on_layer("WALLS"));
        d.add(Entity::line(p(0., 5.), p(10., 5.)).on_layer("WALLS"));
        d.add(Entity::circle(p(50., 50.), 4.0).on_layer("DOORS"));
        d.add(Entity::text(p(0., 20.), 2.5, "Room 1"));
    });
    assert_eq!(ev(&s, "(sslength (ssget \"X\"))"), "4");
    assert_eq!(ev(&s, "(sslength (ssget \"X\" '((0 . \"LINE\"))))"), "2");
    assert_eq!(ev(&s, "(sslength (ssget \"X\" '((8 . \"WALLS,DOORS\"))))"), "3");
    assert_eq!(ev(&s, "(sslength (ssget \"X\" '((0 . \"LINE,CIRCLE\") (8 . \"D*\"))))"), "1");
    assert_eq!(ev(&s, "(sslength (ssget \"X\" '((1 . \"Room*\"))))"), "1");
    assert_eq!(ev(&s, "(ssget \"X\" '((0 . \"ARC\")))"), "nil");
    assert_eq!(ev(&s, "(sslength (ssget \"W\" '(-1 -1) '(11 6)))"), "2");
    assert_eq!(ev(&s, "(sslength (ssget \"C\" '(5 -5) '(6 1)))"), "1");
    // iterate over a set
    ev(&s, "(setq ss (ssget \"X\" '((0 . \"LINE\"))) i 0 total 0)");
    ev(&s, "(while (< i (sslength ss)) (setq g (entget (ssname ss i)) total (+ total (cadr (assoc 11 g))) i (1+ i)))");
    assert_eq!(ev(&s, "!total"), "20.0");
    // ssadd / ssdel / ssmemb
    ev(&s, "(setq a (ssname ss 0) b (ssname ss 1) set2 (ssadd a))");
    ev(&s, "(ssadd b set2)");
    assert_eq!(ev(&s, "(sslength set2)"), "2");
    ev(&s, "(ssdel a set2)");
    assert_eq!(ev(&s, "(sslength set2)"), "1");
    assert_eq!(ev(&s, "(if (ssmemb b set2) \"yes\" \"no\")"), "\"yes\"");
}

#[test]
fn get_functions_wait_for_the_user() {
    let s = Sim::new();
    ev(&s, "(setq pt1 (getpoint \"\\nPick a point: \"))");
    assert!(s.busy());
    assert_eq!(s.message(), "Pick a point:");
    s.click(12., 7.);
    assert_eq!(ev(&s, "!pt1"), "(12.0 7.0 0.0)");
    ev(&s, "(setq d (getdist pt1 \"Distance: \"))");
    s.click(12., 17.);
    assert_eq!(ev(&s, "!d"), "10.0");
    ev(&s, "(setq r (getreal \"Radius: \"))");
    s.ty("2.5");
    assert_eq!(ev(&s, "!r"), "2.5");
    ev(&s, "(setq n (getint \"Count: \"))");
    s.ty("4");
    assert_eq!(ev(&s, "!n"), "4");
    ev(&s, "(setq name (getstring \"Name: \"))");
    s.ty("Alice");
    assert_eq!(ev(&s, "!name"), "\"Alice\"");
    ev(&s, "(initget 1 \"Yes No\")");
    ev(&s, "(setq k (getkword \"Continue? [Yes/No]: \"))");
    s.ty("no");
    assert_eq!(ev(&s, "!k"), "\"No\"");
    // Enter gives nil
    ev(&s, "(setq q (getpoint \"Point: \"))");
    s.enter();
    assert_eq!(ev(&s, "!q"), "nil");
}

#[test]
fn escape_cancels_the_routine_and_runs_the_error_handler() {
    let s = Sim::new();
    ev(&s, "(defun *error* (m) (setq msg m) (princ))");
    ev(&s, "(defun c:ask () (setq p (getpoint \"Point: \")) (setq after \"ran\"))");
    s.ty("ASK");
    assert!(s.busy());
    s.esc();
    assert!(!s.busy());
    assert_eq!(ev(&s, "!after"), "nil", "the routine stopped");
    assert_eq!(ev(&s, "!msg"), "\"Function cancelled\"", "*error* is told about the cancel, as in AutoLISP");
}

#[test]
fn defun_c_name_defines_a_command() {
    let s = Sim::new();
    ev(&s, "(defun c:box2 (/ w) (setq w 4) (command \"rectang\" '(0 0) (list w w)) (princ \"done\"))");
    assert!(s.registry().find("BOX2").is_some(), "the new command is registered");
    s.ty("box2");
    assert_eq!(s.n(), 1);
    assert!(s.history_has("done"));
    // repeats like any other command
    s.enter();
    assert_eq!(s.n(), 2);
    // its aliases and suggestions
    assert!(s.registry().suggest("BOX", false, &[], 10).contains(&"BOX2".to_string()));
}

#[test]
fn getvar_setvar_and_trans() {
    let s = Sim::new();
    assert_eq!(ev(&s, "(getvar \"LTSCALE\")"), "1.0");
    ev(&s, "(setvar \"LTSCALE\" 2.5)");
    assert_eq!(ev(&s, "(getvar \"LTSCALE\")"), "2.5");
    assert_eq!(ev(&s, "(type (getvar \"OSMODE\"))"), "INT");
    assert!(ev(&s, "(setvar \"NOSUCH\" 1)").contains("error"));
    ev(&s, "(command \"UCS\" \"Move\" '(10 10))");
    assert_eq!(ev(&s, "(trans '(0 0 0) 1 0)"), "(10.0 10.0 0.0)", "UCS origin in world coordinates");
    assert_eq!(ev(&s, "(trans '(10 10 0) 0 1)"), "(0.0 0.0 0.0)");
}

#[test]
fn geometry_helpers() {
    let s = Sim::new();
    assert_eq!(ev(&s, "(distance '(0 0) '(3 4))"), "5.0");
    assert_eq!(ev(&s, "(angle '(0 0) '(0 1))"), "1.5708");
    assert_eq!(ev(&s, "(polar '(1 1) 0 5)"), "(6.0 1.0 0.0)");
    assert_eq!(ev(&s, "(inters '(0 0) '(10 10) '(0 10) '(10 0))"), "(5.0 5.0 0.0)");
    assert_eq!(ev(&s, "(inters '(0 0) '(1 1) '(0 10) '(10 0))"), "nil");
    assert_eq!(ev(&s, "(inters '(0 0) '(1 1) '(0 10) '(10 0) nil)"), "(5.0 5.0 0.0)");
}

#[test]
fn a_realistic_routine_draws_a_bolt_circle() {
    let s = Sim::new();
    ev(&s, "(defun bolt-circle (cx cy r n / i ang)\n(setq i 0)\n(while (< i n) (setq ang (* i (/ (* 2 pi) n))) (command \"circle\" (polar (list cx cy) ang r) 1.0) (setq i (1+ i))))");
    ev(&s, "(bolt-circle 50.0 50.0 20.0 6)");
    assert_eq!(s.n(), 6);
    let Kind::Circle { c, .. } = s.ent(3).kind else { panic!() };
    near(c.x, 30.0);
    near(c.y, 50.0);
}
