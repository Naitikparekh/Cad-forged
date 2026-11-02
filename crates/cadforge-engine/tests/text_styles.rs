//! Text styles: -STYLE defines a style with a font, fixed height, width factor and oblique angle; it becomes current
//! (TEXTSTYLE), and TEXT and MTEXT then use it.
use cadforge_core::entities::*;
use cadforge_core::text::TextData;
use cadforge_engine::sim::Sim;

fn texts(s: &Sim) -> Vec<TextData> {
    s.ents().into_iter().filter_map(|e| if let Kind::Text(t) = e.kind { Some(t) } else { None }).collect()
}

#[test]
fn style_command_defines_a_current_style() {
    let s = Sim::new();
    s.cmd("-STYLE").ty("Notes").ty("Times New Roman").ty("3.5").ty("0.8").ty("15");
    assert!(s.history_has("Text style \"Notes\" is current."), "{}", s.log_text());
    let st = s.doc(|d| d.settings.cur_text());
    assert_eq!((st.name.as_str(), st.font.as_str(), st.height, st.width, st.oblique), ("Notes", "Times New Roman", 3.5, 0.8, 15.0));
    // TEXT with a fixed-height style does not ask for a height
    s.cmd("TEXT").click(0., 0.).ty("0").ty("HELLO").enter();
    let t = texts(&s);
    assert_eq!(t.len(), 1, "{}", s.log_text());
    assert_eq!(t[0].style.as_deref(), Some("Notes"));
    assert_eq!(t[0].h, 3.5);
    let f = t[0].font.clone().expect("the style's font is applied");
    assert_eq!((f.face.as_str(), f.width, f.oblique), ("times new roman", 0.8, 15.0));
}

#[test]
fn changing_a_style_redraws_its_text_and_undo_restores_it() {
    let s = Sim::new();
    s.cmd("-STYLE").ty("Mono").ty("Courier New").ty("0").ty("1").ty("0");
    s.cmd("TEXT").click(0., 0.).ty("2").ty("0").ty("A").enter();
    assert_eq!(texts(&s)[0].font.as_ref().map(|f| f.face.clone()).as_deref(), Some("courier new"), "{}", s.log_text());
    s.cmd("-STYLE").ty("Mono").ty("Consolas").ty("0").ty("1.2").ty("0");
    let f = texts(&s)[0].font.clone().unwrap();
    assert_eq!((f.face.as_str(), f.width), ("consolas", 1.2));
    s.undo();
    assert_eq!(texts(&s)[0].font.as_ref().map(|f| f.face.clone()).as_deref(), Some("courier new"));
}

#[test]
fn textstyle_variable_and_text_style_option() {
    let s = Sim::new();
    s.cmd("-STYLE").ty("A").ty("arial.ttf").ty("0").ty("1").ty("0");
    s.cmd("SETVAR").ty("TEXTSTYLE").ty("Standard");
    assert_eq!(s.doc(|d| d.settings.cur_text().name), "Standard", "{}", s.log_text());
    s.cmd("SETVAR").ty("TEXTSTYLE").ty("Nope");
    assert_eq!(s.doc(|d| d.settings.cur_text().name), "Standard", "an unknown style is refused");
    // TEXT's Style option picks a style
    s.cmd("TEXT").ty("S").ty("a").click(0., 0.).ty("2").ty("0").ty("X").enter();
    assert_eq!(texts(&s)[0].style.as_deref(), Some("A"), "{}", s.log_text());
    // Standard text stores no style and draws in the default font
    s.cmd("SETVAR").ty("TEXTSTYLE").ty("Standard");
    s.cmd("TEXT").click(0., 10.).ty("2").ty("0").ty("Y").enter();
    let t = texts(&s);
    assert!(t[1].style.is_none() && t[1].font.is_none());
}

#[test]
fn lisp_finds_every_text_style() {
    let s = Sim::new();
    s.cmd("-STYLE").ty("Notes").ty("romans.shx").ty("2").ty("0.9").ty("0");
    let before = s.h.borrow().history.len();
    s.ty("(cdr (assoc 3 (tblsearch \"STYLE\" \"notes\")))");
    let out: Vec<String> = s.h.borrow().history.iter().skip(before).map(|l| l.text.clone()).collect();
    assert!(out.iter().any(|l| l.contains("\"romans.shx\"")), "{out:?}");
}
