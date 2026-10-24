//! Robustness: malformed input must never panic or hang (part of the AutoCAD parity benchmark).
use cadforge_core::drawing::Drawing;
use cadforge_core::io::{read_dxf, write_dxf};
use cadforge_core::sample::sample_drawing;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() as usize) % n.max(1)
    }
}

fn mutate(src: &str, rng: &mut Rng) -> String {
    let mut lines: Vec<String> = src.lines().map(|l| l.to_string()).collect();
    let ops = 1 + rng.below(12);
    for _ in 0..ops {
        if lines.is_empty() {
            break;
        }
        let i = rng.below(lines.len());
        match rng.below(7) {
            0 => {
                lines.remove(i);
            }
            1 => lines[i] = "NaN".into(),
            2 => lines[i] = "1e999".into(),
            3 => lines[i] = String::from_utf8_lossy(&[0xff, 0xfe, 0x41]).into_owned(),
            4 => {
                let j = rng.below(lines.len());
                lines.swap(i, j);
            }
            5 => lines.truncate(i),
            _ => lines[i] = format!("{}", rng.next() as i64 - (1 << 30)),
        }
    }
    lines.join("\n")
}

#[test]
fn mutated_dxf_never_panics() {
    let base = write_dxf(&sample_drawing());
    let mut rng = Rng(0xC0FFEE);
    let iters: usize = std::env::var("FUZZ_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400);
    for i in 0..iters {
        let m = mutate(&base, &mut rng);
        if std::env::var("FUZZ_DUMP").is_ok() {
            std::fs::write(std::env::temp_dir().join("fuzz-current.dxf"), &m).unwrap();
            eprintln!("iteration {i}");
        }
        let r = std::panic::catch_unwind(|| read_dxf(&m, "fuzz"));
        assert!(r.is_ok(), "read_dxf panicked on a mutated file");
    }
}

#[test]
fn garbage_and_truncated_dxf_is_rejected_cleanly() {
    assert!(read_dxf("", "x").is_err());
    assert!(read_dxf("hello world", "x").is_err());
    let base = write_dxf(&sample_drawing());
    for cut in [10, 100, 1000, base.len() / 2, base.len() - 3] {
        let _ = std::panic::catch_unwind(|| read_dxf(&base[..cut.min(base.len())], "x"))
            .expect("no panic on truncation");
    }
}

#[test]
fn mutated_project_json_never_panics() {
    let base = sample_drawing().to_json_string();
    let mut rng = Rng(7);
    let bytes = base.as_bytes();
    for _ in 0..300 {
        let mut b = bytes.to_vec();
        for _ in 0..1 + rng.below(8) {
            let i = rng.below(b.len());
            match rng.below(3) {
                0 => b[i] = b'0' + (rng.below(10) as u8),
                1 => {
                    b.remove(i);
                }
                _ => b[i] = b'-',
            }
        }
        let s = String::from_utf8_lossy(&b).into_owned();
        let r = std::panic::catch_unwind(|| Drawing::from_json_str(&s, None));
        assert!(r.is_ok(), "from_json_str panicked");
    }
}
