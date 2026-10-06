//! Text layout shared by rendering, hit-testing, bounds and plotting (port of `src/core/text.js`).
use crate::geom::*;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

pub type MeasureFn = fn(&str, f64) -> f64;

fn default_measure(s: &str, h: f64) -> f64 {
    s.chars().count() as f64 * h * 0.62
}

static MEASURE: RwLock<MeasureFn> = RwLock::new(default_measure);

/// Install a glyph-metrics based width function (width in drawing units of `str` at text cap height `h`).
pub fn set_measure(f: MeasureFn) {
    *MEASURE.write().unwrap() = f;
}

pub fn measure_text(s: &str, h: f64) -> f64 {
    (*MEASURE.read().unwrap())(s, h)
}

pub const CAP_RATIO: f64 = 0.72;
pub const LINE_PITCH: f64 = 1.5;
pub fn em_size(h: f64) -> f64 {
    h / CAP_RATIO
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HJust {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VJust {
    #[default]
    Baseline,
    Bottom,
    Middle,
    Top,
}

/// Text entity data (also produced for dimension and leader text).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextData {
    pub p: Pt,
    pub h: f64,
    pub s: String,
    #[serde(default)]
    pub rot: f64,
    #[serde(default)]
    pub jh: HJust,
    #[serde(default)]
    pub jv: VJust,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub multi: bool,
}

impl TextData {
    pub fn new(p: Pt, h: f64, s: &str) -> TextData {
        TextData { p, h, s: s.to_string(), rot: 0.0, jh: HJust::Left, jv: VJust::Baseline, w: None, multi: false }
    }
    pub fn with_rot(mut self, rot: f64) -> TextData {
        self.rot = rot;
        self
    }
    pub fn with_just(mut self, jh: HJust, jv: VJust) -> TextData {
        self.jh = jh;
        self.jv = jv;
        self
    }
}

/// Break text to lines, honouring explicit newlines and (optionally) wrapping at width `w`.
pub fn wrap_lines(s: &str, h: f64, w: f64) -> Vec<String> {
    let raw: Vec<String> = s.replace("\\P", "\n").split('\n').map(|x| x.to_string()).collect();
    if !(w > 0.0) {
        return raw;
    }
    let mut out = vec![];
    for line in raw {
        let mut cur = String::new();
        for word in line.split(' ') {
            let next = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
            if !cur.is_empty() && measure_text(&next, h) > w {
                out.push(std::mem::take(&mut cur));
                cur = word.to_string();
            } else {
                cur = next;
            }
        }
        out.push(cur);
    }
    out
}

#[derive(Clone, Debug)]
pub struct LineLayout {
    pub s: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub lines: Vec<LineLayout>,
    /// Local bounding box (x0, y0, x1, y1).
    pub bx: (f64, f64, f64, f64),
    pub em: f64,
}

/// Local layout of a text entity. Local origin = insertion point, +x along the baseline.
pub fn text_layout(e: &TextData) -> Layout {
    let h = e.h;
    let lines = wrap_lines(&e.s, h, e.w.unwrap_or(0.0));
    let widths: Vec<f64> = lines.iter().map(|l| measure_text(l, h)).collect();
    let max_w = widths.iter().cloned().fold(0.0, f64::max);
    let ew = e.w.unwrap_or(0.0);
    let box_w = if ew > 0.0 { ew.max(max_w) } else { max_w };
    let pitch = h * LINE_PITCH;
    let n = lines.len() as f64;
    let block_h = (n - 1.0) * pitch + h;
    let y0 = match e.jv {
        VJust::Top => -h,
        VJust::Middle => block_h / 2.0 - h,
        VJust::Bottom => (n - 1.0) * pitch,
        VJust::Baseline => 0.0,
    };
    let ox = match e.jh {
        HJust::Center => -box_w / 2.0,
        HJust::Right => -box_w,
        HJust::Left => 0.0,
    };
    let out: Vec<LineLayout> = lines
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let w = widths[i];
            let x = match e.jh {
                HJust::Center => ox + (box_w - w) / 2.0,
                HJust::Right => ox + box_w - w,
                HJust::Left => ox,
            };
            LineLayout { s, x, y: y0 - i as f64 * pitch, w }
        })
        .collect();
    let top = y0 + h;
    let bottom = y0 - (n - 1.0) * pitch - h * 0.3;
    Layout { lines: out, bx: (ox, bottom, ox + box_w, top), em: em_size(h) }
}

fn local_matrix(e: &TextData) -> Mat {
    mat::mul(&mat::translate(e.p.x, e.p.y), &mat::rotate(e.rot, Pt::ZERO))
}

/// World-space corners (CCW) of the text bounding box.
pub fn text_corners(e: &TextData) -> [Pt; 4] {
    let l = text_layout(e);
    let (x0, y0, x1, y1) = l.bx;
    let m = local_matrix(e);
    [pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1)].map(|p| mat::apply(&m, p))
}

/// World-space start of each line's baseline: (text, point, width).
pub fn text_line_origins(e: &TextData) -> Vec<(String, Pt, f64)> {
    let l = text_layout(e);
    let m = local_matrix(e);
    l.lines.into_iter().map(|ln| (ln.s, mat::apply(&m, pt(ln.x, ln.y)), ln.w)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_alignment_and_box() {
        let t = TextData::new(Pt::ZERO, 2.0, "HELLO");
        let l = text_layout(&t);
        assert_eq!(l.lines.len(), 1);
        let w = measure_text("HELLO", 2.0);
        assert!((l.bx.2 - w).abs() < 1e-12 && l.bx.0 == 0.0);
        let c = text_layout(&t.clone().with_just(HJust::Center, VJust::Middle));
        assert!((c.lines[0].x + w / 2.0).abs() < 1e-12);
        assert!((c.lines[0].y + 2.0 / 2.0).abs() < 1e-12);     // middle: baseline sits h/2 below the insertion point
        let r = text_layout(&t.clone().with_just(HJust::Right, VJust::Top));
        assert!((r.lines[0].x + w).abs() < 1e-12 && (r.lines[0].y + 2.0).abs() < 1e-12);
    }

    #[test]
    fn multiline_and_wrapping() {
        let mut t = TextData::new(Pt::ZERO, 2.0, "one\ntwo\\Pthree");
        assert_eq!(text_layout(&t).lines.len(), 3);
        t.w = Some(measure_text("alpha beta", 2.0) * 0.9);
        t.s = "alpha beta gamma".into();
        let l = text_layout(&t);
        assert!(l.lines.len() >= 2);
        assert!(l.lines.iter().all(|x| x.w <= t.w.unwrap() + 1e-9));
        let corners = text_corners(&t.clone().with_rot(PI / 2.0));
        let min_x = corners.iter().map(|c| c.x).fold(f64::INFINITY, f64::min);
        assert!((min_x + 2.0).abs() < 1e-9);           // rotated 90° CCW: the cap height (2.0) extends towards -x
    }

    #[test]
    fn json_matches_the_web_format() {
        let t: TextData = serde_json::from_str(r#"{"p":{"x":1,"y":2},"h":2.5,"s":"Hi","rot":0.5,"jh":"center","jv":"middle"}"#).unwrap();
        assert_eq!((t.jh, t.jv), (HJust::Center, VJust::Middle));
        assert_eq!(t.w, None);
        let v = serde_json::to_value(&t).unwrap();
        assert!(v.get("w").is_none() && v.get("multi").is_none() && v["jh"] == "center");
    }
}
