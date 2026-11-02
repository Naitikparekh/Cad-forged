//! Paragraph formatting of multiline text: alignment, bullets and numbering (AutoCAD's `\p` codes, which carry on from
//! paragraph to paragraph), tabs, wrapping at the text width, and the width grip.
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::pt;
use cadforge_core::io::{read_dxf, write_dxf};
use cadforge_core::mtext::{join_paragraphs, parse_paras, split_paragraphs, Align, List, ParaFmt};
use cadforge_core::text::{text_layout, TextData};

fn mtext(s: &str, w: f64) -> TextData {
    let Kind::Text(t) = Entity::mtext(pt(0., 0.), 1.0, s, w).kind else { panic!() };
    t
}

#[test]
fn paragraph_codes_are_properties_not_breaks() {
    let p = parse_paras("\\pxqc;Title\\P\\pxi-1.5,l1.5,t1.5,ql;•\tone\\Ptwo", 1.0, true);
    assert_eq!(p.len(), 3, "\\p is not a paragraph break");
    assert_eq!(p[0].0.align, Some(Align::Center));
    assert_eq!((p[1].0.first, p[1].0.left, p[1].0.tabs.clone(), p[1].0.align), (-1.5, 1.5, vec![1.5], Some(Align::Left)));
    assert_eq!(p[2].0.left, 1.5, "properties carry on to the next paragraph");
    let text: String = p[0].1.iter().map(|r| if let cadforge_core::mtext::Piece::Text(t) = &r.piece { t.clone() } else { String::new() }).collect();
    assert_eq!(text, "Title", "the code's argument is not drawn");
}

#[test]
fn centred_and_right_paragraphs_sit_in_the_width() {
    let l = text_layout(&mtext("\\pxqc;ab\\P\\pxqr;ab\\P\\pxql;ab", 20.0));
    let (c, r, left) = (&l.lines[0], &l.lines[1], &l.lines[2]);
    assert!((c.x - (20.0 - c.w) / 2.0).abs() < 1e-9, "centred: {}", c.x);
    assert!((r.x + r.w - 20.0).abs() < 1e-9, "right: {}", r.x);
    assert!(left.x.abs() < 1e-9);
}

#[test]
fn bullets_hang_and_wrapped_lines_line_up_after_the_bullet() {
    let s = join_paragraphs(&[(ParaFmt { align: Align::Left, list: List::Bullet }, "alpha beta gamma delta epsilon zeta eta theta".into())]);
    let l = text_layout(&mtext(&s, 12.0));
    assert!(l.lines.len() >= 2, "the item wraps: {:?}", l.lines);
    // the bullet starts at the margin; the text after the tab and every wrapped line start at the indent (1.5 h)
    assert!(l.lines[0].x.abs() < 1e-9);
    let first_word = l.runs.iter().find(|r| r.s.starts_with("alpha")).unwrap();
    assert!((first_word.x - 1.5).abs() < 1e-9, "{}", first_word.x);
    assert!((l.lines[1].x - 1.5).abs() < 1e-9, "{}", l.lines[1].x);
    assert!(l.lines.iter().all(|ln| ln.x + ln.w <= 12.0 + 1e-6), "every line fits the width");
}

#[test]
fn justified_lines_reach_both_edges_except_the_last() {
    let l = text_layout(&mtext("\\pxqj;one two three four five six seven eight nine ten", 15.0));
    assert!(l.lines.len() >= 2);
    let first_line_runs: Vec<_> = l.runs.iter().filter(|r| (r.y - l.lines[0].y).abs() < 1e-9).collect();
    let end = first_line_runs.last().map(|r| r.x + cadforge_core::text::measure_text(r.s.trim_end(), r.h)).unwrap();
    assert!((end - 15.0).abs() < 1e-6, "the first line reaches the right edge: {end}");
    let last_y = l.lines.last().unwrap().y;
    let last_end = l.runs.iter().filter(|r| (r.y - last_y).abs() < 1e-9).map(|r| r.x + r.w).fold(0.0, f64::max);
    assert!(last_end < 15.0 - 1e-6, "the last line stays left");
}

#[test]
fn editor_paragraphs_round_trip() {
    let paras = vec![
        (ParaFmt { align: Align::Center, list: List::None }, "Legend".to_string()),
        (ParaFmt { align: Align::Left, list: List::Bullet }, "Green light = Motor starter is on".to_string()),
        (ParaFmt { align: Align::Left, list: List::Number }, "first".to_string()),
        (ParaFmt { align: Align::Left, list: List::Number }, "second".to_string()),
        (ParaFmt::default(), "plain {\\Lunderlined} note".to_string()),
    ];
    let s = join_paragraphs(&paras);
    assert!(s.contains("2.\t"), "numbers count up: {s}");
    assert_eq!(split_paragraphs(&s), paras);
    // plain paragraphs stay plain text
    let plain = vec![(ParaFmt::default(), "a".to_string()), (ParaFmt::default(), "b".to_string())];
    assert_eq!(join_paragraphs(&plain), "a\nb");
    assert_eq!(split_paragraphs("a\\Pb"), plain);
}

#[test]
fn formatted_paragraphs_survive_dxf() {
    let s = join_paragraphs(&[(ParaFmt { align: Align::Right, list: List::None }, "Title".into()), (ParaFmt { align: Align::Left, list: List::Bullet }, "item".into())]);
    let mut d = Drawing::new("p");
    d.add(Entity::mtext(pt(0., 0.), 2.5, &s, 40.0));
    let back = read_dxf(&write_dxf(&d), "p.dxf").unwrap();
    let Kind::Text(t) = &back.model.entities[0].kind else { panic!() };
    assert_eq!(t.s, s, "codes and the tab come back");
    assert_eq!(t.w, Some(40.0));
}

#[test]
fn the_width_grip_sets_the_wrap_width() {
    let d = Drawing::new("g");
    let e = Entity::mtext(pt(10., 5.), 1.0, "Green light = Motor starter is on", 8.0);
    let g = grips_of(&e, &d).into_iter().find(|g| g.id == "w").expect("a width grip");
    assert!((g.pt.x - 18.0).abs() < 1e-9, "on the right edge: {:?}", g.pt);
    let wider = grip_edit(&e, "w", pt(40., g.pt.y), &d);
    let Kind::Text(t) = &wider.kind else { panic!() };
    assert_eq!(t.w, Some(30.0));
    assert_eq!(text_layout(t).lines.len(), 1, "wide enough for one line");
    let narrow = grip_edit(&e, "w", pt(10.2, g.pt.y), &d);
    let Kind::Text(t) = &narrow.kind else { panic!() };
    assert_eq!(t.w, Some(1.0), "never narrower than one text height");
}
