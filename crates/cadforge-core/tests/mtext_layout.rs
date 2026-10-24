//! Layout of formatted multiline text: positions, wrapping, fractions, bounds, and that plain text is untouched.
use cadforge_core::geom::*;
use cadforge_core::mtext::Style;
use cadforge_core::text::*;

fn mtext(s: &str, h: f64, w: Option<f64>) -> TextData {
    let mut t = TextData::new(Pt::ZERO, h, s);
    t.multi = true;
    t.w = w;
    t
}

#[test]
fn plain_text_takes_the_old_path_and_has_no_runs() {
    let l = text_layout(&mtext("one\ntwo three", 2.5, None));
    assert!(l.runs.is_empty() && l.bars.is_empty());
    assert_eq!(l.lines.len(), 2);
    assert_eq!(l.lines[0].s, "one");
}

#[test]
fn formatting_does_not_move_text_that_is_not_formatted() {
    // the same words with and without a harmless group: identical line positions
    let plain = text_layout(&mtext("one\\Ptwo three", 2.5, None));
    let grouped = text_layout(&mtext("{one}\\Ptwo three", 2.5, None));
    assert!(grouped.runs.len() >= 2);
    assert_eq!(plain.lines.len(), grouped.lines.len());
    for (a, b) in plain.lines.iter().zip(&grouped.lines) {
        assert!((a.y - b.y).abs() < 1e-9 && (a.x - b.x).abs() < 1e-9 && (a.w - b.w).abs() < 1e-9 && a.s == b.s, "{a:?} vs {b:?}");
    }
    assert!((plain.bx.1 - grouped.bx.1).abs() < 1e-9 && (plain.bx.3 - grouped.bx.3).abs() < 1e-9);
}

#[test]
fn runs_follow_each_other_on_the_baseline_and_keep_their_style() {
    let l = text_layout(&mtext("a {\\fArial|b1;bold} {\\C1;red}", 2.5, None));
    let r: Vec<(&str, bool, bool)> = l.runs.iter().map(|r| (r.s.as_str(), r.style.bold, r.style.color.is_some())).collect();
    assert_eq!(r, vec![("a ", false, false), ("bold", true, false), (" ", false, false), ("red", false, true)]);
    for w in l.runs.windows(2) {
        assert!((w[0].x + w[0].w - w[1].x).abs() < 1e-9, "each run starts where the last ended");
        assert!((w[0].y - w[1].y).abs() < 1e-12);
    }
    assert_eq!(l.lines.len(), 1);
    assert_eq!(l.lines[0].s, "a bold red");
}

#[test]
fn a_taller_run_makes_its_line_taller_and_spaces_the_lines() {
    let l = text_layout(&mtext("small\\P\\H2x;BIG\\Pend", 2.5, None));
    assert_eq!(l.lines.len(), 3);
    let (y0, y1, y2) = (l.lines[0].y, l.lines[1].y, l.lines[2].y);
    assert!(y0 - y1 > 2.5 * LINE_PITCH * 1.9, "the gap above a double-height line is double: {}", y0 - y1);
    assert!((y0 - y1 - (y1 - y2)).abs() < 1e-9, "and the gap below it too (the later lines are still sized by the big style: it carries on)");
    let big = l.runs.iter().find(|r| r.s == "BIG").unwrap();
    assert!((big.h - 5.0).abs() < 1e-9);
}

#[test]
fn stacked_fractions_have_a_bar_and_two_smaller_texts() {
    let l = text_layout(&mtext("x \\S1/2; y \\S+1^-1; z", 2.5, None));
    assert_eq!(l.bars.len(), 1, "only the fraction form has a bar; the tolerance form has none");
    let nums: Vec<&RunLayout> = l.runs.iter().filter(|r| (r.h - 2.5 * 0.7).abs() < 1e-9).collect();
    assert_eq!(nums.len(), 4);
    let (num, den) = (nums[0], nums[1]);
    assert_eq!((num.s.as_str(), den.s.as_str()), ("1", "2"));
    let (bx0, bx1, by) = l.bars[0];
    assert!(num.y > by && den.y + den.h < by, "numerator above the bar, denominator below it");
    assert!(num.x >= bx0 - 1e-9 && num.x + num.w <= bx1 + 1e-9);
    // the bounding box is tall enough for the fraction
    assert!(l.bx.3 >= num.y + num.h - 1e-9 && l.bx.1 <= den.y - den.h * 0.3 + 1e-9);
    assert!(l.lines[0].s.contains("1/2"));
}

#[test]
fn wrapping_flows_words_across_runs_and_never_splits_a_word() {
    let t = mtext("alpha {\\C1;beta} gamma delta epsilon", 2.5, Some(30.0));
    let l = text_layout(&t);
    assert!(l.lines.len() >= 2, "{:?}", l.lines);
    for line in &l.lines {
        assert!(line.w <= 30.0 + 1e-9 || !line.s.contains(' '), "{line:?}");
    }
    let joined: Vec<&str> = l.lines.iter().flat_map(|x| x.s.split(' ')).collect();
    assert_eq!(joined, vec!["alpha", "beta", "gamma", "delta", "epsilon"]);
    // a coloured word is one run even after wrapping
    assert_eq!(l.runs.iter().filter(|r| r.style.color.is_some()).count(), 1);
}

#[test]
fn justification_uses_the_formatted_width() {
    let mut t = mtext("{\\C1;abc} defgh", 2.5, None);
    let left = text_layout(&t);
    t.jh = HJust::Right;
    let right = text_layout(&t);
    let w = left.lines[0].w;
    assert!(w > 5.0);
    assert!((right.lines[0].x + w).abs() < 1e-9, "right-justified text ends at the insertion point");
    t.jh = HJust::Center;
    let c = text_layout(&t);
    assert!((c.lines[0].x + w / 2.0).abs() < 1e-9);
    t.jv = VJust::Top;
    let top = text_layout(&t);
    assert!(top.lines[0].y < 0.0, "top-justified: the first baseline is below the insertion point");
}

#[test]
fn percent_codes_apply_to_single_line_text_too() {
    let t = TextData::new(Pt::ZERO, 2.5, "Dia %%c25 at 45%%d");
    let l = text_layout(&t);
    assert_eq!(l.lines[0].s, "Dia Ø25 at 45°");
    assert!(!l.runs.is_empty(), "text with percent codes goes through the formatted layout");
    // but backslash codes are not interpreted in single-line text
    let plain = text_layout(&TextData::new(Pt::ZERO, 2.5, "C:\\temp\\{x}"));
    assert!(plain.runs.is_empty());
    assert_eq!(plain.lines[0].s, "C:\\temp\\{x}");
    let _ = Style::default();
}

#[test]
fn empty_and_odd_input_do_not_panic() {
    for s in ["", "\\P", "{}", "\\Pa\\P\\P", "{{{", "}}}", "\\S/;", "\\H0;x", "\\H-3;x", "\\fx|b1;", "   ", "\\L"] {
        let mut t = mtext(s, 2.5, None);
        for w in [None, Some(10.0)] {
            t.w = w;
            for jv in [VJust::Baseline, VJust::Top, VJust::Middle, VJust::Bottom] {
                t.jv = jv;
                let l = text_layout(&t);
                assert!(l.bx.0 <= l.bx.2 && l.bx.1 <= l.bx.3, "{s:?}: {:?}", l.bx);
                assert!(l.bx.0.is_finite() && l.bx.1.is_finite() && l.bx.2.is_finite() && l.bx.3.is_finite());
            }
        }
    }
}

// ---------------------------------------------------------------- DXF

use cadforge_core::entities::Kind;
use cadforge_core::io::{read_dxf_bytes, write_dxf};
use std::path::PathBuf;
use std::process::Command;

const FORMATTED: &str = "{\\fArial|b1;Bold} and \\C1;red\\P\\Lunder\\l \\S1/2; {\\fArial|i1;slant}";

fn ezdxf() -> bool {
    Command::new("python3").args(["-I", "-c", "import ezdxf"]).output().map(|o| o.status.success()).unwrap_or(false)
}

fn mtext_of(d: &cadforge_core::drawing::Drawing) -> TextData {
    d.model.entities.iter().find_map(|e| if let Kind::Text(t) = &e.kind { t.multi.then(|| t.clone()) } else { None }).expect("an MTEXT")
}

#[test]
fn an_mtext_made_by_ezdxf_keeps_its_formatting_on_import() {
    if !ezdxf() {
        eprintln!("ezdxf not installed; skipping");
        return;
    }
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mtext-in.dxf");
    let script = "import sys, ezdxf\nd = ezdxf.new('R2018')\nd.modelspace().add_mtext(sys.argv[2], dxfattribs={'char_height': 2.5, 'insert': (10, 20)})\nd.saveas(sys.argv[1])";
    let out = Command::new("python3").args(["-I", "-c", script, path.to_str().unwrap(), FORMATTED]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let d = read_dxf_bytes(&std::fs::read(&path).unwrap(), "in").unwrap();
    let t = mtext_of(&d);
    // paragraph codes become newlines, every other code is kept for the layout
    assert_eq!(t.s, FORMATTED.replace("\\P", "\n"));
    let l = text_layout(&t);
    assert_eq!(l.lines.len(), 2);
    assert_eq!(l.lines[0].s, "Bold and red");
    assert_eq!(l.lines[1].s, "under 1/2 slant");
    assert!(l.runs.iter().any(|r| r.s == "Bold" && r.style.bold));
    assert!(l.runs.iter().any(|r| r.s == "red" && r.style.color.is_some()));
    assert!(l.runs.iter().any(|r| r.s == "under" && r.style.underline));
    assert!(l.runs.iter().any(|r| r.s == "slant" && r.style.italic));
    assert_eq!(l.bars.len(), 1);
}

#[test]
fn our_export_writes_the_codes_back_and_ezdxf_reads_the_same_text() {
    if !ezdxf() {
        eprintln!("ezdxf not installed; skipping");
        return;
    }
    let mut d = cadforge_core::drawing::Drawing::new("t");
    d.add(cadforge_core::entities::Entity::mtext(pt(5.0, 5.0), 2.5, &FORMATTED.replace("\\P", "\n"), 0.0));
    let back = read_dxf_bytes(write_dxf(&d).as_bytes(), "back").unwrap();
    assert_eq!(mtext_of(&back).s, mtext_of(&d).s, "import after export gives the same string");
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mtext-out.dxf");
    std::fs::write(&path, write_dxf(&d)).unwrap();
    let script = "import sys, ezdxf\nfrom ezdxf import recover\ndoc, aud = recover.readfile(sys.argv[1])\nassert not aud.has_errors, [e.message for e in aud.errors][:5]\nm = list(doc.modelspace().query('MTEXT'))[0]\nimport json\nprint(json.dumps(m.text))\nprint(json.dumps(m.plain_text()))";
    let out = Command::new("python3").args(["-I", "-c", script, path.to_str().unwrap()]).output().unwrap();
    assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut lines = text.lines();
    assert_eq!(lines.next().unwrap(), serde_json::to_string(FORMATTED).unwrap(), "ezdxf sees the original codes: {text}");
    let plain = lines.next().unwrap();
    assert!(plain.contains("Bold and red") && plain.contains("under") && plain.contains("1/2") && plain.contains("slant"), "{plain}");
}
