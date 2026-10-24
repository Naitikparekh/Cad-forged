//! DWG reading. The samples are AutoCAD drawings from ACadSharp's test suite (MIT, see tests/dwg/samples), one per
//! version from R14 to 2018. AutoCAD also saved each as DXF; reading a DWG must give what reading its DXF twin gives.
//! The 2004 twin is kept whole; for the others `expected.txt` holds a fingerprint of the twin (entity count and a
//! hash of the rounded entity data per space, the layers, layouts and blocks). To rebuild it from the twins
//! (sample_AC*_ascii.dxf in ACadSharp's samples folder):
//! `DWG_TWINS=<folder> cargo test -p cadforge-core --test dwg -- print_expected --nocapture`.
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::Entity;
use cadforge_core::io::dwg::dwg_to_dxf;
use cadforge_core::io::{is_dwg, read_dwg, read_dxf_bytes};
use std::collections::BTreeMap;
use std::path::PathBuf;

const VERSIONS: [&str; 7] = [
    "AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032",
];

fn samples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/dwg/samples")
}

fn dwg(v: &str) -> Vec<u8> {
    std::fs::read(samples().join(format!("sample_{v}.dwg"))).unwrap()
}

/// An entity as JSON with numbers rounded to 9 significant digits (and ids dropped), for comparing readings.
fn canon(e: &Entity) -> String {
    fn round(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    let r = if f.abs() < 1e-9 {
                        0.0
                    } else {
                        let k = 10f64.powi(9 - f.abs().log10().ceil() as i32);
                        (f * k).round() / k
                    };
                    *v = serde_json::json!(r);
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(round),
            serde_json::Value::Object(o) => o.values_mut().for_each(round),
            _ => {}
        }
    }
    let mut e = e.clone();
    e.id = 0;
    e.grp = None;
    let mut j = serde_json::to_value(&e).unwrap();
    round(&mut j);
    j.to_string()
}

fn sorted(es: &[Entity]) -> Vec<String> {
    let mut v: Vec<String> = es.iter().map(canon).collect();
    v.sort();
    v
}

/// FNV-1a: a hash that stays the same across Rust versions.
fn fnv(parts: &[String]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.bytes().chain([0u8]) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

/// One line per drawing: what the comparison checks.
fn fingerprint(d: &Drawing) -> BTreeMap<String, String> {
    let mut f = BTreeMap::new();
    let model = sorted(&d.model.entities);
    f.insert("model".into(), format!("{}:{}", model.len(), fnv(&model)));
    let layers: Vec<String> = d
        .layers
        .iter()
        .map(|l| serde_json::to_string(l).unwrap())
        .collect();
    f.insert(
        "layers".into(),
        format!("{}:{}", layers.len(), fnv(&layers)),
    );
    f.insert("layouts".into(), d.layout_names().join(","));
    for l in &d.layouts {
        let p = sorted(&l.entities);
        f.insert(
            format!("paper.{}", l.name),
            format!("{}:{}", p.len(), fnv(&p)),
        );
    }
    let blocks: Vec<String> = d.blocks.keys().cloned().collect();
    f.insert("blocks".into(), blocks.join(","));
    f
}

fn line(f: &BTreeMap<String, String>) -> String {
    f.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn expected() -> BTreeMap<String, BTreeMap<String, String>> {
    let text = std::fs::read_to_string(samples().join("../expected.txt")).unwrap();
    let mut out = BTreeMap::new();
    for l in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let mut it = l.split(' ');
        let v = it.next().unwrap().to_string();
        let f = it
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        out.insert(v, f);
    }
    out
}

#[test]
fn print_expected() {
    let Ok(dir) = std::env::var("DWG_TWINS") else {
        return;
    };
    for v in VERSIONS {
        if let Ok(b) = std::fs::read(PathBuf::from(&dir).join(format!("sample_{v}_ascii.dxf"))) {
            println!(
                "{v} {}",
                line(&fingerprint(&read_dxf_bytes(&b, "twin").unwrap()))
            );
        }
    }
}

#[test]
fn every_version_reads_like_its_dxf_twin() {
    let want = expected();
    for v in VERSIONS {
        let bytes = dwg(v);
        assert!(is_dwg(&bytes), "{v}");
        let (_, rep) = dwg_to_dxf(&bytes).unwrap();
        assert!(
            rep.failed.is_empty(),
            "{v}: objects that could not be decoded: {:?}",
            rep.failed
        );
        let d = read_dwg(&bytes, v).unwrap();
        let got = fingerprint(&d);
        let Some(exp) = want.get(v) else {
            // R14: AutoCAD wrote no DXF twin; it must still give the 2000 file's model space and layers
            let r2000 = fingerprint(&read_dwg(&dwg("AC1015"), "AC1015").unwrap());
            let n =
                |f: &BTreeMap<String, String>| f["model"].split(':').next().unwrap().to_string();
            assert_eq!(n(&got), n(&r2000), "{v} model entity count");
            assert_eq!(
                got["layers"].split(':').next(),
                r2000["layers"].split(':').next(),
                "{v} layers"
            );
            continue;
        };
        for (k, e) in exp {
            if k == "blocks" {
                // the DWG files also hold blocks AutoCAD leaves out of older DXF (dynamic block definitions)
                for b in e.split(',').filter(|b| !b.is_empty()) {
                    assert!(d.blocks.contains_key(b), "{v}: block {b} missing");
                }
            } else {
                assert_eq!(got.get(k), Some(e), "{v} {k}");
            }
        }
    }
}

#[test]
fn the_2004_file_matches_its_twin_entity_by_entity() {
    let d = read_dwg(&dwg("AC1018"), "AC1018").unwrap();
    let twin = read_dxf_bytes(
        &std::fs::read(samples().join("sample_AC1018_ascii.dxf")).unwrap(),
        "twin",
    )
    .unwrap();
    let (a, b) = (sorted(&d.model.entities), sorted(&twin.model.entities));
    assert_eq!(a.len(), b.len());
    assert!(a.len() > 200, "{}", a.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x, y);
    }
    assert_eq!(
        serde_json::to_string(&d.layers).unwrap(),
        serde_json::to_string(&twin.layers).unwrap()
    );
    assert_eq!(d.layout_names(), twin.layout_names());
    // the kinds of content in the sample, all read: text with formatting, hatches, splines, multileaders, images
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for e in &d.model.entities {
        *kinds.entry(e.type_name()).or_default() += 1;
    }
    for k in [
        "line", "arc", "circle", "ellipse", "pline", "spline", "hatch", "text", "insert", "leader",
        "image", "wipeout", "point", "ray", "xline",
    ] {
        assert!(kinds.contains_key(k), "no {k} in {kinds:?}");
    }
    assert_eq!(kinds["leader"], 16, "1 leader + 15 multileaders");
}

#[test]
fn unsupported_content_is_reported_not_dropped_silently() {
    let d = read_dwg(&dwg("AC1032"), "AC1032").unwrap();
    for k in ["3DSOLID", "REGION", "ACAD_TABLE", "TOLERANCE"] {
        assert!(
            d.skipped.contains_key(k),
            "{k} not reported: {:?}",
            d.skipped
        );
    }
    assert!(
        !d.skipped.keys().any(|k| k.contains("unreadable")),
        "{:?}",
        d.skipped
    );
}

#[test]
fn not_dwg_and_unknown_versions_are_refused() {
    assert!(!is_dwg(b"0\nSECTION\n"));
    assert!(read_dwg(b"0\nSECTION\n", "x").is_err());
    let mut b = dwg("AC1015");
    b[4..6].copy_from_slice(b"99");
    assert!(read_dwg(&b, "x").is_err());
    assert!(read_dwg(b"AC10", "x").is_err());
}

/// Truncated and corrupted files: an error or a partial drawing, never a panic or a hang.
#[test]
fn damaged_files_do_not_panic() {
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for v in VERSIONS {
        let good = dwg(v);
        for cut in [
            16,
            0x80,
            0x100,
            0x500,
            good.len() / 3,
            good.len() / 2,
            good.len() - 100,
        ] {
            let _ = read_dwg(&good[..cut], v);
        }
        // DWG_FUZZ_ROUNDS raises the count for a longer run
        let rounds: usize = std::env::var("DWG_FUZZ_ROUNDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(16);
        for _ in 0..rounds {
            let mut b = good.clone();
            for _ in 0..1 + next() % 40 {
                let at = (next() as usize) % b.len();
                b[at] = next() as u8;
            }
            let _ = read_dwg(&b, v);
        }
        // a run of zeros or ones over the middle of the file
        for fill in [0u8, 0xFF] {
            let mut b = good.clone();
            let at = b.len() / 2;
            b[at..at + 4096].fill(fill);
            let _ = read_dwg(&b, v);
        }
    }
}
