//! The AutoLISP interpreter, driven the way a user does: expressions typed at the command line.
use cadforge_engine::sim::Sim;

/// Type `src` at the command line and return what the history shows for it (the lines after the echoed input).
fn run(s: &Sim, src: &str) -> String {
    let before = s.h.borrow().history.len();
    s.ty(src);
    let h = s.h.borrow();
    h.history.iter().skip(before).map(|l| l.text.clone()).filter(|t| !t.starts_with("Command:")).collect::<Vec<_>>().join("\n")
}

fn ev(s: &Sim, src: &str) -> String {
    run(s, src)
}

fn check(pairs: &[(&str, &str)]) {
    let s = Sim::new();
    for (src, want) in pairs {
        assert_eq!(ev(&s, src), *want, "{src}");
    }
}

#[test]
fn arithmetic_follows_autolisp_rules() {
    check(&[
        ("(+ 1 2)", "3"),
        ("(+ 1 2.5)", "3.5"),
        ("(/ 5 2)", "2"),
        ("(/ 5 2.0)", "2.5"),
        ("(/ -7 2)", "-3"),
        ("(* 2 3.5)", "7.0"),
        ("(- 5)", "-5"),
        ("(- 10 3 2)", "5"),
        ("(1+ 4)", "5"),
        ("(1- 4)", "3"),
        ("(sqrt 16)", "4.0"),
        ("(expt 2 10)", "1024"),
        ("(expt 2 0.5)", "1.41421"),
        ("(fix 3.9)", "3"),
        ("(fix -3.9)", "-3"),
        ("(float 3)", "3.0"),
        ("(rem 7 3)", "1"),
        ("(abs -2.5)", "2.5"),
        ("(max 1 5 3)", "5"),
        ("(min 2.5 1)", "1.0"),
        ("(atan 1 1)", "0.785398"),
        ("(* 4 (atan 1))", "3.14159"),
        ("(logand 12 10)", "8"),
        ("(logior 12 10)", "14"),
        ("(lsh 1 4)", "16"),
        ("!pi", "3.14159"),
        ("(/ 1 3.0)", "0.333333"),
        ("(* 1000000.0 1000000.0)", "1.0e+012"),
        ("(/ 1 100000.0)", "1.0e-005"),
        ("(= 1 1.0)", "T"),
        ("(< 1 2 3)", "T"),
        ("(< 1 3 2)", "nil"),
        ("(/= 1 2)", "T"),
        ("(equal 1.0 1.0000001 0.001)", "T"),
    ]);
}

#[test]
fn arithmetic_errors_use_autolisp_messages() {
    let s = Sim::new();
    assert!(ev(&s, "(/ 1 0)").contains("; error: divide by zero"));
    assert!(ev(&s, "(+ 1 \"a\")").contains("; error: bad argument type: numberp \"a\""));
    assert!(ev(&s, "(nosuchfn 1)").contains("; error: no function definition: NOSUCHFN"));
    assert!(ev(&s, "(car 5)").contains("bad argument type: listp 5"));
    assert!(ev(&s, "(+ 1").contains("1>") || s.busy(), "an unfinished expression asks for more");
}

#[test]
fn strings() {
    check(&[
        ("(strcat \"a\" \"b\" \"c\")", "\"abc\""),
        ("(substr \"hello\" 2 3)", "\"ell\""),
        ("(substr \"hello\" 4)", "\"lo\""),
        ("(strlen \"abc\")", "3"),
        ("(strcase \"aB\")", "\"AB\""),
        ("(strcase \"aB\" T)", "\"ab\""),
        ("(rtos 3.14159 2 2)", "\"3.14\""),
        ("(rtos 1234.5 1 3)", "\"1.234E+03\""),
        ("(rtos 18.5 5 2)", "\"18-1/2\""),
        ("(atoi \"12x\")", "12"),
        ("(atof \"2.5abc\")", "2.5"),
        ("(itoa 7)", "\"7\""),
        ("(chr 65)", "\"A\""),
        ("(ascii \"A\")", "65"),
        ("(wcmatch \"Wall-1\" \"Wall*\")", "T"),
        ("(wcmatch \"Wall-1\" \"Door*,Wall-#\")", "T"),
        ("(wcmatch \"abc\" \"~a*\")", "nil"),
        ("(vl-string-search \"lo\" \"hello\")", "3"),
        ("(vl-string-subst \"J\" \"l\" \"hello\")", "\"heJlo\""),
        ("(vl-string-trim \" \" \"  hi \")", "\"hi\""),
        ("(angtos 0.785398 0 2)", "\"45.00\""),
        ("(angtos (/ pi 2) 1 0)", "\"90d\""),
        ("(read \"(1 2)\")", "(1 2)"),
        ("(vl-princ-to-string '(1 \"a\"))", "\"(1 a)\""),
        ("(vl-prin1-to-string '(1 \"a\"))", "\"(1 \\\"a\\\")\""),
    ]);
}

#[test]
fn lists() {
    check(&[
        ("(car '(1 2 3))", "1"),
        ("(cdr '(1 2 3))", "(2 3)"),
        ("(cadr '(1 2 3))", "2"),
        ("(caddr '(1 2 3))", "3"),
        ("(cddr '(1 2 3))", "(3)"),
        ("(cons 1 '(2))", "(1 2)"),
        ("(cons 1 2)", "(1 . 2)"),
        ("(list 1 \"a\" 2.5)", "(1 \"a\" 2.5)"),
        ("(nth 2 '(a b c))", "C"),
        ("(nth 9 '(a b c))", "nil"),
        ("(last '(1 2 3))", "3"),
        ("(length '(1 2 3))", "3"),
        ("(append '(1) '(2 3) nil '(4))", "(1 2 3 4)"),
        ("(reverse '(1 2 3))", "(3 2 1)"),
        ("(member 2 '(1 2 3))", "(2 3)"),
        ("(member 9 '(1 2 3))", "nil"),
        ("(assoc 'b '((a . 1) (b . 2)))", "(B . 2)"),
        ("(cdr (assoc 2 '((1 . \"x\") (2 . \"y\"))))", "\"y\""),
        ("(subst 9 2 '(1 2 3 2))", "(1 9 3 9)"),
        ("(mapcar '1+ '(1 2 3))", "(2 3 4)"),
        ("(mapcar '+ '(1 2) '(10 20))", "(11 22)"),
        ("(mapcar (function (lambda (x) (* x x))) '(1 2 3))", "(1 4 9)"),
        ("(apply '+ '(1 2 3))", "6"),
        ("(vl-remove-if-not 'numberp '(1 a 2))", "(1 2)"),
        ("(vl-remove-if 'numberp '(1 a 2))", "(A)"),
        ("(vl-sort '(3 1 2) '<)", "(1 2 3)"),
        ("(vl-position 3 '(1 2 3))", "2"),
        ("(vl-some 'minusp '(1 -2))", "T"),
        ("(vl-every 'numberp '(1 a))", "nil"),
        ("(listp '(1))", "T"),
        ("(atom 'a)", "T"),
        ("(null nil)", "T"),
        ("(type 1)", "INT"),
        ("(type 1.5)", "REAL"),
        ("(type \"a\")", "STR"),
        ("(type '(1))", "LIST"),
        ("(type nil)", "nil"),
    ]);
}

#[test]
fn control_flow_functions_and_scope() {
    let s = Sim::new();
    assert_eq!(ev(&s, "(defun fact (n) (if (<= n 1) 1 (* n (fact (1- n)))))"), "FACT");
    assert_eq!(ev(&s, "(fact 10)"), "3628800");
    ev(&s, "(setq total 0)");
    ev(&s, "(foreach i '(1 2 3 4) (setq total (+ total i)))");
    assert_eq!(ev(&s, "!total"), "10");
    ev(&s, "(setq i 0 acc nil)");
    ev(&s, "(while (< i 3) (setq acc (cons i acc) i (1+ i)))");
    assert_eq!(ev(&s, "!acc"), "(2 1 0)");
    assert_eq!(ev(&s, "(repeat 3 (setq i (1+ i)))"), "6");
    assert_eq!(ev(&s, "(cond ((= 1 2) \"a\") ((= 2 2) \"b\") (T \"c\"))"), "\"b\"");
    assert_eq!(ev(&s, "(cond (nil 1) (5))"), "5", "a clause without a body returns its test");
    assert_eq!(ev(&s, "(and 1 2 3)"), "3");
    assert_eq!(ev(&s, "(or nil nil)"), "nil");
    // locals are restored after the call, globals are not touched
    ev(&s, "(setq x 1)");
    ev(&s, "(defun f (a / x) (setq x (* a 10)) x)");
    assert_eq!(ev(&s, "(f 4)"), "40");
    assert_eq!(ev(&s, "!x"), "1");
    // dynamic scope: a callee sees the caller's local
    ev(&s, "(defun g () (+ y 1))");
    ev(&s, "(defun h (/ y) (setq y 41) (g))");
    assert_eq!(ev(&s, "(h)"), "42");
    // lambda values and functions as arguments
    ev(&s, "(defun twice (fn v) (fn (fn v)))");
    assert_eq!(ev(&s, "(twice '1+ 5)"), "7");
    assert_eq!(ev(&s, "(foreach v (quote (1 2)) (princ v))"), "12\n2", "foreach prints, then the echo shows its last value");
}

#[test]
fn error_handler_and_catch_all() {
    let s = Sim::new();
    ev(&s, "(defun *error* (msg) (setq seen msg) (princ))");
    ev(&s, "(/ 1 0)");
    assert_eq!(ev(&s, "!seen"), "\"divide by zero\"");
    ev(&s, "(setq r (vl-catch-all-apply '/ '(1 0)))");
    assert_eq!(ev(&s, "(vl-catch-all-error-p r)"), "T");
    assert_eq!(ev(&s, "(vl-catch-all-error-message r)"), "\"divide by zero\"");
    assert_eq!(ev(&s, "(vl-catch-all-apply '+ '(1 2))"), "3");
}

#[test]
fn a_runaway_loop_stops_with_an_error() {
    let s = Sim::new();
    s.h.borrow().lisp.set_step_limit(20_000);
    let out = ev(&s, "(while T (setq a 1))");
    assert!(out.contains("evaluation limit exceeded"), "{out}");
    s.h.borrow().lisp.set_step_limit(cadforge_engine::lisp::DEFAULT_STEP_LIMIT);
    ev(&s, "(defun up (n) (if (> n 0) (+ 1 (up (1- n))) 0))");
    assert_eq!(ev(&s, "(up 300)"), "300");
    assert_eq!(ev(&s, "(up 20000)"), "20000", "deep recursion uses the heap, not the native stack");
    assert!(ev(&s, "(up 1000000)").contains("stack overflow"));
    assert_eq!(ev(&s, "(up 5)"), "5", "the interpreter recovers after a stack overflow");
}

#[test]
fn princ_output_is_not_echoed_twice() {
    let s = Sim::new();
    let out = ev(&s, "(princ \"hello\")");
    assert_eq!(out, "hello");
    let out = ev(&s, "(progn (princ \"a\") (princ \"b\") (princ))");
    assert_eq!(out, "ab");
    let out = ev(&s, "(progn (princ \"line1\\nline2\") 5)");
    assert_eq!(out, "line1\nline2\n5");
    assert!(ev(&s, "(print \"x\")").contains("\"x\""), "print writes the value quoted");
}
