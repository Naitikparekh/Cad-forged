//! Binary DXF: files written by ezdxf read like their text twins, ezdxf reads what we write, and damaged files are
//! refused without a panic.
use cadforge_core::drawing::Drawing;
use cadforge_core::geom::*;
use cadforge_core::io::{binary_dxf, is_binary_dxf, read_dxf, read_dxf_bytes, to_json, write_dxf, write_dxf_binary};
use cadforge_core::sample::sample_drawing;
use std::path::PathBuf;
use std::process::Command;

fn have_ezdxf() -> bool {
    Command::new("python3").args(["-I", "-c", "import ezdxf"]).output().map(|o| o.status.success()).unwrap_or(false)
}

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/dxf/corpus").join(name)
}

fn tmp(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

fn python(script: &str, args: &[&str]) -> String {
    let r = Command::new("python3").arg("-I").arg("-c").arg(script).args(args).output().unwrap();
    assert!(r.status.success(), "{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
    String::from_utf8_lossy(&r.stdout).into_owned()
}

fn doc() -> Drawing {
    let mut d = sample_drawing();
    d.add(cadforge_core::entities::Entity::text(pt(10.0, 10.0), 5.0, "Hello \u{00D8} \u{00FC} binary"));
    d.add(cadforge_core::entities::Entity::circle(pt(50.0, 50.0), 12.5));
    d
}

#[test]
fn the_sentinel_is_recognised_and_text_is_not_binary() {
    let b = write_dxf_binary(&doc());
    assert!(is_binary_dxf(&b));
    assert!(b.starts_with(b"AutoCAD Binary DXF\r\n\x1a\0"));
    assert!(!is_binary_dxf(write_dxf(&doc()).as_bytes()));
    assert!(!is_binary_dxf(b""));
}

#[test]
fn our_binary_file_reads_back_as_the_same_drawing_as_our_text_file() {
    let d = doc();
    let from_text = read_dxf(&write_dxf(&d), "x").unwrap();
    let from_bin = read_dxf_bytes(&write_dxf_binary(&d), "x").unwrap();
    assert_eq!(to_json(&from_text), to_json(&from_bin));
    assert!(from_bin.model.entities.len() >= 10, "{} entities", from_bin.model.entities.len());
    // text outside ASCII survives (the writer escapes it as \U+XXXX in both formats)
    assert!(from_bin.model.entities.iter().any(|e| matches!(&e.kind, cadforge_core::entities::Kind::Text(t) if t.s == "Hello \u{00D8} \u{00FC} binary")));
}

#[test]
fn the_binary_file_is_smaller_than_the_text_file() {
    // not by much: most of a DXF is names and handles, which are strings in both formats
    let d = doc();
    let (t, b) = (write_dxf(&d).len(), write_dxf_binary(&d).len());
    assert!(b < t, "binary {b} bytes, text {t} bytes");
}

#[test]
fn every_group_type_survives_the_conversion() {
    // one group of each type: string, double, int16, int32, int64, flag, binary chunk (200 bytes = two groups)
    let hex: String = (0..200u32).map(|i| format!("{:02X}", (i * 7) % 256)).collect();
    let text = format!("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n40\n-2.5\n70\n-7\n90\n-100000\n160\n-5000000000\n290\n1\n310\n{hex}\n0\nENDSEC\n0\nEOF\n");
    let back = binary_dxf::binary_to_text(&binary_dxf::text_to_binary(&text)).unwrap();
    let groups: Vec<&str> = back.lines().collect();
    let find = |code: &str| groups.iter().position(|l| *l == code).map(|i| groups[i + 1]).unwrap();
    assert_eq!(find("40"), "-2.5");
    assert_eq!(find("70"), "-7");
    assert_eq!(find("90"), "-100000");
    assert_eq!(find("160"), "-5000000000");
    assert_eq!(find("290"), "1");
    assert_eq!(find("1"), "AC1015");
    // the 200 bytes come back as two chunks (127 + 73) that together are the same data
    let chunks: Vec<&str> = groups.windows(2).filter(|w| w[0] == "310").map(|w| w[1]).collect();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks.concat(), hex);
}

#[test]
fn ezdxf_reads_the_binary_file_we_write_and_finds_the_same_drawing() {
    if !have_ezdxf() {
        eprintln!("ezdxf not installed; skipping");
        return;
    }
    let d = doc();
    let (asc, bin) = (tmp("ours-text.dxf"), tmp("ours-binary.dxf"));
    std::fs::write(&asc, write_dxf(&d)).unwrap();
    std::fs::write(&bin, write_dxf_binary(&d)).unwrap();
    let script = "import sys, ezdxf\nout = []\nfor f in sys.argv[1:]:\n    doc = ezdxf.readfile(f)\n    aud = doc.audit()\n    assert not aud.has_errors, [e.message for e in aud.errors][:5]\n    out.append(sorted((e.dxftype(), e.dxf.layer) for e in doc.modelspace()))\n    out.append(sorted((t.dxf.text if t.dxftype() == 'TEXT' else '') for t in doc.modelspace()))\nassert out[0] == out[2], 'entities differ'\nassert out[1] == out[3], 'text differs'\nprint(len(out[0]), 'entities;', ezdxf.readfile(sys.argv[2]).dxfversion)";
    let r = python(script, &[asc.to_str().unwrap(), bin.to_str().unwrap()]);
    assert!(r.contains("entities; AC1015"), "{r}");
}

#[test]
fn files_ezdxf_saves_as_binary_read_exactly_like_their_text_twins() {
    if !have_ezdxf() {
        eprintln!("ezdxf not installed; skipping");
        return;
    }
    let script = "import sys, ezdxf\ndoc = ezdxf.readfile(sys.argv[1])\ndoc.saveas(sys.argv[2], fmt='asc')\ndoc.saveas(sys.argv[3], fmt='bin')";
    let mut n = 0;
    for entry in std::fs::read_dir(corpus("")).unwrap() {
        let p = entry.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if !name.ends_with(".dxf") {
            continue;
        }
        let (asc, bin) = (tmp(&format!("tw-{name}")), tmp(&format!("tb-{name}")));
        python(script, &[p.to_str().unwrap(), asc.to_str().unwrap(), bin.to_str().unwrap()]);
        let bytes = std::fs::read(&bin).unwrap();
        assert!(is_binary_dxf(&bytes), "{name}: ezdxf wrote a binary file");
        let a = read_dxf_bytes(&std::fs::read(&asc).unwrap(), "x").unwrap_or_else(|e| panic!("{name} text: {e}"));
        let b = read_dxf_bytes(&bytes, "x").unwrap_or_else(|e| panic!("{name} binary: {e}"));
        assert_eq!(to_json(&a), to_json(&b), "{name}: the binary file reads differently from its text twin");
        assert!(!a.model.entities.is_empty(), "{name}: has content ({} entities)", a.model.entities.len());
        n += 1;
    }
    assert!(n >= 10, "only {n} corpus files");
}

#[test]
fn an_r12_binary_file_with_one_byte_group_codes_is_read() {
    if !have_ezdxf() {
        eprintln!("ezdxf not installed; skipping");
        return;
    }
    let script = "import sys, ezdxf\ndoc = ezdxf.new('R12')\nmsp = doc.modelspace()\nmsp.add_line((0, 0), (10, 5))\nmsp.add_circle((3, 4), 2.5)\nmsp.add_text('R12 text', dxfattribs={'height': 2, 'insert': (1, 1)})\ndoc.saveas(sys.argv[1], fmt='bin')\ndoc.saveas(sys.argv[2], fmt='asc')";
    let (bin, asc) = (tmp("r12.bin.dxf"), tmp("r12.asc.dxf"));
    python(script, &[bin.to_str().unwrap(), asc.to_str().unwrap()]);
    let bytes = std::fs::read(&bin).unwrap();
    assert!(is_binary_dxf(&bytes));
    let b = read_dxf_bytes(&bytes, "r12").unwrap();
    let a = read_dxf_bytes(&std::fs::read(&asc).unwrap(), "r12").unwrap();
    assert_eq!(to_json(&a), to_json(&b));
    let types: Vec<&str> = b.model.entities.iter().map(|e| e.type_name()).collect();
    assert_eq!(types, vec!["line", "circle", "text"], "{types:?}");
}

#[test]
fn a_damaged_binary_file_is_an_error_not_a_crash() {
    let good = write_dxf_binary(&doc());
    // cut at every length: either a clean error or a (partial) drawing, never a panic
    for cut in (0..good.len()).step_by(7).chain([good.len() - 1]) {
        let r = std::panic::catch_unwind(|| read_dxf_bytes(&good[..cut], "x"));
        assert!(r.is_ok(), "panicked at {cut} bytes");
    }
    // a value that runs past the end is reported
    let mut cut = good.clone();
    cut.truncate(good.len() - 3);
    cut.extend_from_slice(&[0x28, 0x00, 0x01]); // code 40 (a double) with only one byte of its value
    let e = binary_dxf::binary_to_text(&cut).unwrap_err();
    assert!(e.contains("ends in the middle"), "{e}");
    // random damage
    let mut seed = 0x2545F4914F6CDD1Du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..300 {
        let mut m = good.clone();
        for _ in 0..1 + next() % 6 {
            let i = 22 + (next() as usize) % (m.len() - 22);
            m[i] = next() as u8;
        }
        let r = std::panic::catch_unwind(|| read_dxf_bytes(&m, "x"));
        assert!(r.is_ok(), "panicked on a damaged file");
    }
    assert!(binary_dxf::binary_to_text(b"AutoCAD Binary DXF").is_err());
    assert!(read_dxf_bytes(b"AutoCAD Binary DXF\r\n\x1a\0", "x").is_err(), "a header with no drawing in it");
}
