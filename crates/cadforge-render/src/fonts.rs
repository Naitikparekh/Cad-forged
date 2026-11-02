//! Fonts for drawing text. A text style names a font the way AutoCAD files do (`arial.ttf`, `Times New Roman`,
//! `romans.shx`); [`resolve`] turns that into a font CadForge can draw with:
//!
//! * a TrueType / OpenType font installed on this computer, found by file name or family name (desktop only), or
//! * one of three bundled faces (Liberation Sans, Serif and Mono, SIL OFL 1.1), chosen by the name's look: Times and
//!   other serif names to Serif, Courier and other fixed-pitch names to Mono, everything else (Arial, Helvetica and
//!   the SHX fonts such as `romans.shx`, `simplex.shx`, `txt.shx`, `isocp.shx`) to Sans.
//!
//! SHX fonts are not read: AutoCAD's own SHX files are not free to ship, so their text is drawn in the bundled Sans.
use ab_glyph::{Font, FontArc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub static SANS: &[u8] = include_bytes!("../../cadforge-app/assets/fonts/LiberationSans-Regular.ttf");
pub static SERIF: &[u8] = include_bytes!("../../cadforge-app/assets/fonts/LiberationSerif-Regular.ttf");
pub static MONO: &[u8] = include_bytes!("../../cadforge-app/assets/fonts/LiberationMono-Regular.ttf");

/// One of the bundled faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    Sans,
    Serif,
    Mono,
}

impl Builtin {
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Builtin::Sans => SANS,
            Builtin::Serif => SERIF,
            Builtin::Mono => MONO,
        }
    }
    /// The PDF standard font with the same metrics.
    pub fn pdf_name(self) -> &'static str {
        match self {
            Builtin::Sans => "Helvetica",
            Builtin::Serif => "Times-Roman",
            Builtin::Mono => "Courier",
        }
    }
    /// A CSS font stack for SVG.
    pub fn css(self) -> &'static str {
        match self {
            Builtin::Sans => "Helvetica, Arial, 'Liberation Sans', sans-serif",
            Builtin::Serif => "'Times New Roman', Times, 'Liberation Serif', serif",
            Builtin::Mono => "'Courier New', Courier, 'Liberation Mono', monospace",
        }
    }
}

/// The bundled face whose look is closest to the font named `face` (a `face_key`).
pub fn builtin_for(face: &str) -> Builtin {
    let f = face.replace(['-', '_'], " ");
    // SHX fonts of AutoCAD: the Roman complex / duplex / triplex ones are serifed, the rest are plain
    const SHX_SERIF: [&str; 6] = ["romanc", "romand", "romant", "italicc", "italict", "scriptc"];
    if SHX_SERIF.contains(&f.as_str()) {
        return Builtin::Serif;
    }
    if f.contains("mono") || f.contains("cour") || f.contains("consol") || f.contains("lucida console") || f.contains("fixed") || f == "monotxt" {
        return Builtin::Mono;
    }
    let serif = ["times", "serif", "georgia", "garamond", "cambria", "book antiqua", "palatino", "century", "bodoni", "baskerville", "caladea", "tinos"];
    if serif.iter().any(|s| f.contains(s)) && !f.contains("sans") {
        return Builtin::Serif;
    }
    Builtin::Sans
}

/// What a font name resolves to.
#[derive(Clone, Debug)]
pub enum Resolved {
    Builtin(Builtin),
    /// An installed font: its key in the system font index and the font itself.
    System(String, FontArc),
}

impl Resolved {
    pub fn font(&self) -> FontArc {
        match self {
            Resolved::Builtin(b) => builtin_font(*b),
            Resolved::System(_, f) => f.clone(),
        }
    }
    /// The bundled face standing in for it (PDF and SVG use the standard fonts for these).
    pub fn builtin(&self) -> Option<Builtin> {
        match self {
            Resolved::Builtin(b) => Some(*b),
            Resolved::System(..) => None,
        }
    }
}

fn builtin_font(b: Builtin) -> FontArc {
    static FONTS: OnceLock<[FontArc; 3]> = OnceLock::new();
    let f = FONTS.get_or_init(|| [Builtin::Sans, Builtin::Serif, Builtin::Mono].map(|b| FontArc::try_from_slice(b.bytes()).expect("bundled font")));
    f[b as usize].clone()
}

/// Installed fonts: a lower-case name (file name without extension, and family name) to its file.
fn system_index() -> &'static HashMap<String, std::path::PathBuf> {
    &system_fonts().0
}

/// The index and the family names as the fonts spell them (regular faces), sorted.
fn system_fonts() -> &'static (HashMap<String, std::path::PathBuf>, Vec<String>) {
    static INDEX: OnceLock<(HashMap<String, std::path::PathBuf>, Vec<String>)> = OnceLock::new();
    INDEX.get_or_init(scan_system_fonts)
}

#[cfg(not(target_arch = "wasm32"))]
fn font_dirs() -> Vec<std::path::PathBuf> {
    let mut v: Vec<std::path::PathBuf> = vec![];
    if let Some(w) = std::env::var_os("WINDIR") {
        v.push(std::path::PathBuf::from(w).join("Fonts"));
    }
    if let Some(l) = std::env::var_os("LOCALAPPDATA") {
        v.push(std::path::PathBuf::from(l).join("Microsoft").join("Windows").join("Fonts"));
    }
    if let Some(h) = std::env::var_os("HOME") {
        let h = std::path::PathBuf::from(h);
        v.push(h.join("Library/Fonts"));
        v.push(h.join(".fonts"));
        v.push(h.join(".local/share/fonts"));
    }
    v.extend(["/Library/Fonts", "/System/Library/Fonts", "/System/Library/Fonts/Supplemental", "/usr/share/fonts", "/usr/local/share/fonts"].map(std::path::PathBuf::from));
    v
}

#[cfg(not(target_arch = "wasm32"))]
fn scan_system_fonts() -> (HashMap<String, std::path::PathBuf>, Vec<String>) {
    let mut out: HashMap<String, std::path::PathBuf> = HashMap::new();
    let mut families: Vec<(String, std::path::PathBuf, bool)> = vec![];
    let mut stack = font_dirs();
    let mut seen = 0usize;
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
            if ext != "ttf" && ext != "otf" {
                continue;
            }
            seen += 1;
            if seen > 5000 {
                break;
            }
            let stem = p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            out.entry(stem).or_insert_with(|| p.clone());
            // the family name, from the font's name table (regular faces win over bold and italic ones)
            if let Ok(bytes) = std::fs::read(&p) {
                if let Ok(face) = ttf_parser::Face::parse(&bytes, 0) {
                    let name_of = |id: u16| face.names().into_iter().filter(|n| n.name_id == id && n.is_unicode()).find_map(|n| n.to_string());
                    if let Some(fam) = name_of(ttf_parser::name_id::FAMILY) {
                        let regular = name_of(ttf_parser::name_id::SUBFAMILY).is_none_or(|s| s.eq_ignore_ascii_case("regular") || s.eq_ignore_ascii_case("normal") || s.eq_ignore_ascii_case("roman"));
                        families.push((fam, p.clone(), regular));
                    }
                }
            }
        }
    }
    let mut names: Vec<String> = vec![];
    for (fam, p, regular) in families {
        let key = fam.to_lowercase();
        if regular || !out.contains_key(&key) {
            out.insert(key, p);
        }
        if !names.iter().any(|n| n.eq_ignore_ascii_case(&fam)) {
            names.push(fam);
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    (out, names)
}

#[cfg(target_arch = "wasm32")]
fn scan_system_fonts() -> (HashMap<String, std::path::PathBuf>, Vec<String>) {
    (HashMap::new(), vec![])
}

/// Installed font names (families and files), sorted, for a font list.
pub fn system_font_names() -> Vec<String> {
    system_fonts().1.clone()
}

/// The fonts a font list offers before the installed ones: the TrueType names drawings use most and AutoCAD's SHX
/// fonts. Each is drawn with an installed font of that name, or else the bundled face that looks closest.
pub const COMMON_FONTS: [&str; 12] = ["Arial", "Times New Roman", "Courier New", "Calibri", "Verdana", "romans.shx", "simplex.shx", "txt.shx", "isocp.shx", "romand.shx", "romanc.shx", "monotxt.shx"];

/// The font text in face `face` (a `face_key`; empty for the default) is drawn with.
pub fn resolve(face: &str) -> Resolved {
    static CACHE: OnceLock<Mutex<HashMap<String, Resolved>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(r) = cache.lock().expect("font cache").get(face) {
        return r.clone();
    }
    let r = resolve_uncached(face);
    cache.lock().expect("font cache").insert(face.to_string(), r.clone());
    r
}

fn resolve_uncached(face: &str) -> Resolved {
    let f = face.trim().to_lowercase();
    if f.is_empty() {
        return Resolved::Builtin(Builtin::Sans);
    }
    // the default look stays the bundled Sans, so drawings look the same on every computer
    if matches!(f.as_str(), "arial" | "helvetica" | "liberation sans" | "liberationsans" | "txt" | "simplex" | "romans") {
        return Resolved::Builtin(Builtin::Sans);
    }
    if let Some(p) = system_index().get(&f) {
        if let Some(font) = std::fs::read(p).ok().and_then(|b| FontArc::try_from_vec(b).ok()) {
            return Resolved::System(f, font);
        }
    }
    Resolved::Builtin(builtin_for(&f))
}

/// The font's bytes, for a front end that registers fonts itself (egui).
pub fn bytes_of(face: &str) -> Arc<[u8]> {
    let f = face.trim().to_lowercase();
    match resolve(&f) {
        Resolved::Builtin(b) => Arc::from(b.bytes()),
        Resolved::System(k, _) => system_index().get(&k).and_then(|p| std::fs::read(p).ok()).map(Arc::from).unwrap_or_else(|| Arc::from(SANS)),
    }
}

/// Width of `s` at cap height `h` in font `face` (the measure CadForge's text layout uses). The default face keeps
/// the Helvetica metrics PDF output uses.
pub fn measure(s: &str, h: f64, face: &str) -> f64 {
    let r = resolve(face);
    if r.builtin() == Some(Builtin::Sans) {
        return crate::ops::helvetica_width(s, h);
    }
    let font = r.font();
    let upm = font.units_per_em().unwrap_or(1000.0) as f64;
    let em = h / cadforge_core::text::CAP_RATIO;
    let w: f64 = s.chars().map(|c| font.h_advance_unscaled(font.glyph_id(c)) as f64).sum();
    w / upm * em
}

/// The outline of `s` set in `font` as closed loops in device space: the glyphs of a text op (baseline start `(x, y)`
/// in text-local units, `m` the local-to-device map, `em` the font size). Curves are flattened.
pub fn text_outline(font: &FontArc, s: &str, x: f64, y: f64, m: &[f64; 6], em: f64) -> Vec<Vec<[f64; 2]>> {
    let upm = font.units_per_em().unwrap_or(1000.0) as f64;
    let scale = em / upm;
    let mut pen = x;
    let mut loops: Vec<Vec<[f64; 2]>> = vec![];
    for ch in s.chars() {
        let id = font.glyph_id(ch);
        if let Some(outline) = font.outline(id) {
            let map = |p: ab_glyph::Point| -> [f64; 2] {
                let (lx, ly) = (pen + p.x as f64 * scale, y - p.y as f64 * scale);
                [m[0] * lx + m[2] * ly + m[4], m[1] * lx + m[3] * ly + m[5]]
            };
            let mut cur: Vec<[f64; 2]> = vec![];
            let mut last: Option<ab_glyph::Point> = None;
            for c in &outline.curves {
                let (start, end) = match c {
                    ab_glyph::OutlineCurve::Line(a, b) => (*a, *b),
                    ab_glyph::OutlineCurve::Quad(a, _, b) => (*a, *b),
                    ab_glyph::OutlineCurve::Cubic(a, _, _, b) => (*a, *b),
                };
                let joined = last.is_some_and(|l| (l.x - start.x).abs() < 1e-3 && (l.y - start.y).abs() < 1e-3);
                if !joined {
                    if cur.len() > 2 {
                        loops.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                    cur.push(map(start));
                }
                const N: usize = 6;
                match c {
                    ab_glyph::OutlineCurve::Line(_, b) => cur.push(map(*b)),
                    ab_glyph::OutlineCurve::Quad(a, q, b) => {
                        for i in 1..=N {
                            let t = i as f32 / N as f32;
                            let u = 1.0 - t;
                            cur.push(map(ab_glyph::point(u * u * a.x + 2.0 * u * t * q.x + t * t * b.x, u * u * a.y + 2.0 * u * t * q.y + t * t * b.y)));
                        }
                    }
                    ab_glyph::OutlineCurve::Cubic(a, q1, q2, b) => {
                        for i in 1..=N {
                            let t = i as f32 / N as f32;
                            let u = 1.0 - t;
                            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                            cur.push(map(ab_glyph::point(w[0] * a.x + w[1] * q1.x + w[2] * q2.x + w[3] * b.x, w[0] * a.y + w[1] * q1.y + w[2] * q2.y + w[3] * b.y)));
                        }
                    }
                }
                last = Some(end);
            }
            if cur.len() > 2 {
                loops.push(cur);
            }
        }
        pen += font.h_advance_unscaled(id) as f64 * scale;
    }
    loops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_map_to_the_closest_bundled_face() {
        assert_eq!(builtin_for("times new roman"), Builtin::Serif);
        assert_eq!(builtin_for("times"), Builtin::Serif);
        assert_eq!(builtin_for("cour"), Builtin::Mono);
        assert_eq!(builtin_for("consolas"), Builtin::Mono);
        assert_eq!(builtin_for("romans"), Builtin::Sans, "Roman Simplex is a plain stroke font");
        assert_eq!(builtin_for("romanc"), Builtin::Serif);
        assert_eq!(builtin_for("isocp"), Builtin::Sans);
        assert_eq!(builtin_for("dejavu sans"), Builtin::Sans);
        assert!(matches!(resolve(""), Resolved::Builtin(Builtin::Sans)));
        assert!(matches!(resolve("arial"), Resolved::Builtin(Builtin::Sans)));
    }

    #[test]
    fn widths_follow_the_face() {
        let sans = measure("WWWW", 2.5, "");
        let mono = measure("WWWW", 2.5, "courier new-not-installed");
        let mono_i = measure("iiii", 2.5, "courier new-not-installed");
        assert!((mono - mono_i).abs() < 1e-9, "a fixed-pitch face: every letter is as wide");
        assert!((sans - crate::ops::helvetica_width("WWWW", 2.5)).abs() < 1e-12);
        assert!(measure("Hello", 2.5, "times-not-installed") < sans);
    }
}
