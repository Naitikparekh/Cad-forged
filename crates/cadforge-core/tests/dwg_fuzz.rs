//! Robustness of the DWG reader: damaged files (bytes changed, cut short, runs zeroed) must give an error or a
//! drawing, never a panic or a hang. Uses DWGs CadForge writes, and AutoCAD's own samples when `DWG_TWINS` names their
//! folder.
use cadforge_core::io::{read_dwg, write_dwg};
use cadforge_core::sample::sample_drawing;
use std::time::{Duration, Instant};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() as usize) % n.max(1)
    }
}

fn damage(src: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut b = src.to_vec();
    match rng.below(4) {
        0 => {
            // a few random bytes changed
            for _ in 0..1 + rng.below(8) {
                let i = rng.below(b.len());
                b[i] = rng.next() as u8;
            }
        }
        1 => b.truncate(rng.below(b.len())),
        2 => {
            let i = rng.below(b.len());
            let n = (1 + rng.below(64)).min(b.len() - i);
            b[i..i + n].fill(0);
        }
        _ => {
            let i = rng.below(b.len());
            let n = (1 + rng.below(64)).min(b.len() - i);
            b[i..i + n].fill(0xff);
        }
    }
    b
}

fn sources() -> Vec<(String, Vec<u8>)> {
    let mut v = vec![("sample (CadForge 2004)".to_string(), write_dwg(&sample_drawing()).unwrap())];
    if let Ok(dir) = std::env::var("DWG_TWINS") {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dwg")) {
                    v.push((p.display().to_string(), std::fs::read(&p).unwrap()));
                }
            }
        }
    }
    v
}

#[test]
fn damaged_dwg_never_panics_or_hangs() {
    let mut rng = Rng(0x5eed_d4a9);
    let mut failures = vec![];
    for (name, src) in sources() {
        let runs = if name.starts_with("sample") { 400 } else { 120 };
        for k in 0..runs {
            let bad = damage(&src, &mut rng);
            let t0 = Instant::now();
            let r = std::panic::catch_unwind(|| read_dwg(&bad, "x").is_ok());
            let took = t0.elapsed();
            if r.is_err() {
                failures.push(format!("{name} #{k}: panicked"));
            } else if took > Duration::from_secs(3) {
                failures.push(format!("{name} #{k}: took {took:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{} of the damaged files failed:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn not_a_dwg_is_an_error() {
    for b in [&b""[..], b"AC10", b"AC1032", b"AC1015\0\0\0\0\0\0", &[0u8; 4096][..]] {
        assert!(std::panic::catch_unwind(|| read_dwg(b, "x").is_err()).unwrap_or(false), "{b:?}");
    }
}

/// Every .dwg and .dxf under `DRAWINGS_DIR` opens (a report of the ones that do not).
#[test]
fn real_drawings_open() {
    let Ok(dir) = std::env::var("DRAWINGS_DIR") else { return };
    let mut stack = vec![std::path::PathBuf::from(dir)];
    let mut bad = vec![];
    let mut n = 0;
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
            if ext != "dwg" && ext != "dxf" {
                continue;
            }
            n += 1;
            let bytes = std::fs::read(&p).unwrap();
            let t0 = Instant::now();
            let r = std::panic::catch_unwind(|| if ext == "dwg" { read_dwg(&bytes, "x").map(|d| d.entities().len()) } else { cadforge_core::io::read_dxf_bytes(&bytes, "x").map(|d| d.entities().len()) });
            let took = t0.elapsed();
            match r {
                Ok(Ok(k)) => println!("ok   {:>5} objects {:>6.0?}  {}", k, took, p.display()),
                Ok(Err(e)) => bad.push(format!("ERR  {}: {e}", p.display())),
                Err(_) => bad.push(format!("PANIC {}", p.display())),
            }
        }
    }
    println!("{n} files, {} failed", bad.len());
    for b in &bad {
        println!("{b}");
    }
}

/// Writes damaged sample number `DWG_FUZZ_DUMP` to `DWG_FUZZ_OUT` (for investigating a failure).
#[test]
fn dump_damaged() {
    let (Ok(k), Ok(out)) = (std::env::var("DWG_FUZZ_DUMP"), std::env::var("DWG_FUZZ_OUT")) else { return };
    let k: usize = k.parse().unwrap();
    let mut rng = Rng(0x5eed_d4a9);
    let src = write_dwg(&sample_drawing()).unwrap();
    let mut bad = vec![];
    for _ in 0..=k {
        bad = damage(&src, &mut rng);
    }
    std::fs::write(out, bad).unwrap();
}

/// Reads the file `DWG_READ` names (for investigating a failure).
#[test]
fn read_one() {
    let Ok(p) = std::env::var("DWG_READ") else { return };
    let r = read_dwg(&std::fs::read(p).unwrap(), "x");
    println!("{:?}", r.map(|d| d.entities().len()));
}

/// Object types of a DXF file named by `DXF_TYPES` (for investigating a difference).
#[test]
fn dxf_types() {
    let Ok(p) = std::env::var("DXF_TYPES") else { return };
    let d = cadforge_core::io::read_dxf_bytes(&std::fs::read(p).unwrap(), "x").unwrap();
    let mut m = std::collections::BTreeMap::new();
    for e in d.entities() {
        *m.entry(e.type_name().to_string()).or_insert(0) += 1;
    }
    println!("{m:?} blocks={}", d.blocks.len());
}

/// Writes the binary DXF `BDXF_IN` as text to `BDXF_OUT` (for investigating a difference).
#[test]
fn binary_dxf_as_text() {
    let (Ok(i), Ok(o)) = (std::env::var("BDXF_IN"), std::env::var("BDXF_OUT")) else { return };
    std::fs::write(o, cadforge_core::io::binary_dxf::binary_to_text(&std::fs::read(i).unwrap()).unwrap()).unwrap();
}
