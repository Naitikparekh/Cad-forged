//! Hatch patterns and linetypes loaded from definition files: registration, saving with the drawing, and DXF.
use cadforge_core::defs::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::hatch::{all_patterns, hatch_lines, pattern};
use cadforge_core::io::{read_dxf_bytes, write_dxf};
use cadforge_core::style::*;
use std::path::PathBuf;
use std::process::Command;

fn square(s: f64) -> Vec<Vec<Pt>> {
    vec![vec![pt(0., 0.), pt(s, 0.), pt(s, s), pt(0., s)]]
}

#[test]
fn a_loaded_pattern_hatches_like_a_built_in_one() {
    let p = parse_pat("*DEFS_TEST_BOX, test boxes\n0, 0,2.5, 0,5\n");
    assert_eq!(p.defs.len(), 1);
    let mut d = Drawing::new("t");
    d.define_pattern(p.defs[0].clone()).unwrap();
    assert!(pattern("defs_test_box").is_some(), "found case-insensitively");
    assert!(all_patterns().iter().any(|p| p.name == "DEFS_TEST_BOX"));
    // lines every 5 units starting at y = 2.5 inside a 20 x 20 square: y = 2.5, 7.5, 12.5, 17.5
    let lines = hatch_lines(&square(20.0), "DEFS_TEST_BOX", 1.0, 0.0).unwrap();
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert!(lines.iter().all(|l| (l[2] - l[0]).abs() > 19.9 && l[1] == l[3]));
    // the scale applies
    assert_eq!(hatch_lines(&square(20.0), "DEFS_TEST_BOX", 2.0, 0.0).unwrap().len(), 2);
}

#[test]
fn built_in_names_cannot_be_replaced_but_loaded_ones_can() {
    let mut d = Drawing::new("t");
    let ansi = PatternDef { name: "ansi31".into(), desc: String::new(), families: vec![FamilyDef { a: 0.0, ox: 0.0, oy: 0.0, dx: 0.0, dy: 1.0, dash: vec![] }] };
    assert!(d.define_pattern(ansi).unwrap_err().contains("built-in"));
    let lt = LinetypeDef { name: "dashed".into(), desc: String::new(), pattern: vec![1.0, -1.0] };
    assert!(d.define_linetype(lt).unwrap_err().contains("standard linetype"));
    let fam = |dy| FamilyDef { a: 0.0, ox: 0.0, oy: 0.0, dx: 0.0, dy, dash: vec![] };
    d.define_pattern(PatternDef { name: "defs_test_re".into(), desc: "one".into(), families: vec![fam(10.0)] }).unwrap();
    d.define_pattern(PatternDef { name: "DEFS_TEST_RE".into(), desc: "two".into(), families: vec![fam(2.0)] }).unwrap();
    assert_eq!(d.custom_patterns.len(), 1, "same name = one entry");
    assert_eq!(pattern("DEFS_TEST_RE").unwrap().desc, "two");
    assert!(d.define_pattern(PatternDef { name: "empty".into(), desc: String::new(), families: vec![] }).is_err());
}

#[test]
fn loaded_definitions_are_saved_with_the_drawing() {
    let mut d = Drawing::new("t");
    d.define_pattern(parse_pat("*DEFS_TEST_SAVE, saved\n45, 0,0, 0,3, 2,-1\n").defs.remove(0)).unwrap();
    d.define_linetype(parse_lin("*DEFS_TEST_LT, saved\nA,4,-2,0,-2\n").defs.remove(0)).unwrap();
    d.layers[0].ltype = "DEFS_TEST_LT".into();
    let text = d.to_json_string();
    assert!(text.contains("DEFS_TEST_SAVE") && text.contains("DEFS_TEST_LT"));
    let back = Drawing::from_json_str(&text, None).unwrap();
    assert_eq!(back.custom_patterns, d.custom_patterns);
    assert_eq!(back.custom_ltypes, d.custom_ltypes);
    assert_eq!(back.layers[0].ltype, "DEFS_TEST_LT");
    // a drawing without any has neither key
    assert!(!Drawing::new("x").to_json_string().contains("custom_"));
}

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/dxf/corpus").join(name)
}

#[test]
fn linetypes_defined_in_a_dxf_file_are_kept_by_layers_and_objects() {
    let d = read_dxf_bytes(&std::fs::read(corpus("linetypes-R2018.dxf")).unwrap(), "lt").unwrap();
    let fence = linetype("FENCE_LINE").expect("registered from the LTYPE table");
    assert_eq!(fence.pattern, &[12.0, -6.0]);
    assert_eq!(fence.desc, "Fence ---- ---- ----");
    assert_eq!(linetype("utility_dot").unwrap().pattern, &[6.0, -3.0, 0.0, -3.0]);
    assert_eq!(d.layer("fences").unwrap().ltype, "FENCE_LINE");
    let own: Vec<Option<String>> = d.model.entities.iter().map(|e| e.ltype.clone()).collect();
    assert_eq!(own, vec![None, Some("UTILITY_DOT".to_string())]);
    assert_eq!(d.custom_ltypes.len(), 2, "kept with the drawing");
}

#[test]
fn our_writer_emits_loaded_linetypes_and_ezdxf_reads_them_back() {
    let d = read_dxf_bytes(&std::fs::read(corpus("linetypes-R2018.dxf")).unwrap(), "lt").unwrap();
    let text = write_dxf(&d);
    let back = read_dxf_bytes(text.as_bytes(), "back").unwrap();
    assert_eq!(back.layer("fences").unwrap().ltype, "FENCE_LINE");
    if Command::new("python3").args(["-I", "-c", "import ezdxf"]).output().map(|o| !o.status.success()).unwrap_or(true) {
        eprintln!("ezdxf not installed; skipping the audit");
        return;
    }
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("rt-linetypes.dxf");
    std::fs::write(&out, text).unwrap();
    let script = "import sys, ezdxf\nfrom ezdxf import recover\ndoc, aud = recover.readfile(sys.argv[1])\nassert not aud.has_errors, [e.message for e in aud.errors][:5]\nlt = doc.linetypes.get('FENCE_LINE')\nprint(lt.dxf.name, [t.value for t in lt.pattern_tags.tags if t.code == 49], doc.layers.get('fences').dxf.linetype)";
    let r = Command::new("python3").args(["-I", "-c", script, out.to_str().unwrap()]).output().unwrap();
    assert!(r.status.success(), "{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
    assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), "FENCE_LINE [12.0, -6.0] FENCE_LINE");
}

#[test]
fn a_pattern_used_by_a_hatch_is_written_with_its_line_definitions() {
    let mut d = Drawing::new("t");
    d.define_pattern(parse_pat("*DEFS_TEST_DXF\n0, 0,0, 0,4\n90, 0,0, 0,4, 2,-2\n").defs.remove(0)).unwrap();
    d.add(Entity::hatch(vec![vec![Vertex::new(0., 0., 0.), Vertex::new(10., 0., 0.), Vertex::new(10., 10., 0.), Vertex::new(0., 10., 0.)]], "DEFS_TEST_DXF"));
    let text = write_dxf(&d);
    // (group code, value) pairs of the HATCH record
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let pairs: Vec<(&str, &str)> = lines.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).collect();
    let start = pairs.iter().position(|p| *p == ("2", "DEFS_TEST_DXF")).expect("the pattern name is written");
    let rest = &pairs[start..];
    assert!(rest.contains(&("78", "2")), "two line definitions");
    let angles: Vec<f64> = rest.iter().take_while(|p| p.0 != "98").filter(|p| p.0 == "53").map(|p| p.1.parse().unwrap()).collect();
    assert_eq!(angles, vec![0.0, 90.0]);
    let dashes: Vec<f64> = rest.iter().take_while(|p| p.0 != "98").filter(|p| p.0 == "49").map(|p| p.1.parse().unwrap()).collect();
    assert_eq!(dashes, vec![2.0, -2.0]);
}
