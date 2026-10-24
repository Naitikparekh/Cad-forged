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
    /// Attribute definition (ATTDEF) data when this text is a block attribute template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub att: Option<AttDef>,
}

/// Attribute definition: tag, prompt and mode flags (ATTDEF).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttDef {
    pub tag: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub invisible: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub constant: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub verify: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub preset: bool,
}

impl TextData {
    pub fn new(p: Pt, h: f64, s: &str) -> TextData {
        TextData { p, h, s: s.to_string(), rot: 0.0, jh: HJust::Left, jv: VJust::Baseline, w: None, multi: false, att: None }
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

/// One piece of formatted text, placed on its baseline (local coordinates, y up).
#[derive(Clone, Debug)]
pub struct RunLayout {
    pub s: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    /// Cap height of this run in drawing units.
    pub h: f64,
    pub style: crate::mtext::Style,
}

#[derive(Clone, Debug)]
pub struct Layout {
    /// Plain text per line (also set for formatted text, for code that only needs the lines).
    pub lines: Vec<LineLayout>,
    /// Local bounding box (x0, y0, x1, y1).
    pub bx: (f64, f64, f64, f64),
    pub em: f64,
    /// Formatted text only (bold, colour, stacked fractions...): the pieces to draw instead of `lines`. Empty for plain text.
    pub runs: Vec<RunLayout>,
    /// Fraction bars of stacked text: (x0, x1, y).
    pub bars: Vec<(f64, f64, f64)>,
}

/// Local layout of a text entity. Local origin = insertion point, +x along the baseline.
pub fn text_layout(e: &TextData) -> Layout {
    if crate::mtext::has_formatting(&e.s, e.multi) {
        return layout_formatted(e);
    }
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
    Layout { lines: out, bx: (ox, bottom, ox + box_w, top), em: em_size(h), runs: vec![], bars: vec![] }
}

/// Layout of text with inline formatting: words of every run are flowed into lines (wrapping at the text width), each
/// line's height follows its largest run, stacked fractions take the space of one word.
fn layout_formatted(e: &TextData) -> Layout {
    use crate::mtext::{parse, Piece, Style};
    struct Tok {
        style: Style,
        text: String,
        stack: Option<(String, String, bool)>,
        w: f64,
        trim: f64,
    }
    let h = e.h;
    let wrap = e.w.unwrap_or(0.0);
    let mut lines: Vec<Vec<Tok>> = vec![];
    for para in parse(&e.s, h, e.multi) {
        let (mut cur, mut cur_w): (Vec<Tok>, f64) = (vec![], 0.0);
        for run in para {
            let hh = h * run.style.scale;
            let toks: Vec<Tok> = match run.piece {
                Piece::Text(t) => {
                    // words keep the spaces that follow them
                    let mut words: Vec<String> = vec![];
                    let mut w = String::new();
                    for ch in t.chars() {
                        w.push(ch);
                        if ch == ' ' {
                            words.push(std::mem::take(&mut w));
                        }
                    }
                    if !w.is_empty() {
                        words.push(w);
                    }
                    words.into_iter().map(|w| Tok { style: run.style.clone(), w: measure_text(&w, hh), trim: measure_text(w.trim_end(), hh), text: w, stack: None }).collect()
                }
                Piece::Stack { num, den, bar } => {
                    let sh = hh * 0.7;
                    let w = measure_text(&num, sh).max(measure_text(&den, sh)) + 0.25 * sh;
                    vec![Tok { style: run.style.clone(), text: format!("{num}/{den}"), stack: Some((num, den, bar)), w, trim: w }]
                }
            };
            for t in toks {
                if wrap > 0.0 && !cur.is_empty() && cur_w + t.trim > wrap {
                    lines.push(std::mem::take(&mut cur));
                    cur_w = 0.0;
                }
                cur_w += t.w;
                cur.push(t);
            }
        }
        lines.push(cur);
    }
    let line_w: Vec<f64> = lines.iter().map(|l| l.iter().map(|t| t.w).sum::<f64>() - l.last().map_or(0.0, |t| t.w - t.trim)).collect();
    // a line is as tall as its largest run (a stacked fraction needs about 1.3 times its height)
    let line_scale: Vec<f64> = lines.iter().map(|l| if l.is_empty() { 1.0 } else { l.iter().map(|t| t.style.scale * if t.stack.is_some() { 1.3 } else { 1.0 }).fold(0.0, f64::max) }).collect();
    let n = lines.len();
    let pitches: Vec<f64> = (0..n.saturating_sub(1)).map(|i| h * LINE_PITCH * line_scale[i].max(line_scale[i + 1])).collect();
    let span: f64 = pitches.iter().sum();
    let h0 = h * line_scale[0];
    let block_h = span + h * line_scale[n - 1];
    let y0 = match e.jv {
        VJust::Top => -h0,
        VJust::Middle => block_h / 2.0 - h0,
        VJust::Bottom => span,
        VJust::Baseline => 0.0,
    };
    let max_w = line_w.iter().cloned().fold(0.0, f64::max);
    let ew = e.w.unwrap_or(0.0);
    let box_w = if ew > 0.0 { ew.max(max_w) } else { max_w };
    let ox = match e.jh {
        HJust::Center => -box_w / 2.0,
        HJust::Right => -box_w,
        HJust::Left => 0.0,
    };
    let (mut runs, mut bars, mut out_lines) = (vec![], vec![], vec![]);
    let (mut top, mut bottom) = (y0 + h0, f64::MAX);
    let mut base = y0;
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            base -= pitches[i - 1];
        }
        let x = match e.jh {
            HJust::Center => ox + (box_w - line_w[i]) / 2.0,
            HJust::Right => ox + box_w - line_w[i],
            HJust::Left => ox,
        };
        let mut cx = x;
        let mut plain = String::new();
        bottom = bottom.min(base - h * line_scale[i] * 0.3);
        for t in line {
            let hh = h * t.style.scale;
            plain.push_str(&t.text);
            match &t.stack {
                None => {
                    runs.push(RunLayout { s: t.text.clone(), x: cx, y: base, w: t.w, h: hh, style: t.style.clone() });
                    top = top.max(base + hh);
                    bottom = bottom.min(base - hh * 0.3);
                }
                Some((num, den, bar)) => {
                    let sh = hh * 0.7;
                    let bar_y = base + 0.4 * hh;
                    let (nw, dw) = (measure_text(num, sh), measure_text(den, sh));
                    let (ny, dy) = (bar_y + 0.2 * sh, bar_y - 0.2 * sh - sh);
                    runs.push(RunLayout { s: num.clone(), x: cx + (t.w - nw) / 2.0, y: ny, w: nw, h: sh, style: t.style.clone() });
                    runs.push(RunLayout { s: den.clone(), x: cx + (t.w - dw) / 2.0, y: dy, w: dw, h: sh, style: t.style.clone() });
                    if *bar {
                        bars.push((cx, cx + t.w - 0.1 * sh, bar_y));
                    }
                    top = top.max(ny + sh);
                    bottom = bottom.min(dy - sh * 0.3);
                }
            }
            cx += t.w;
        }
        out_lines.push(LineLayout { s: plain.trim_end().to_string(), x, y: base, w: line_w[i] });
    }
    Layout { lines: out_lines, bx: (ox, bottom, ox + box_w, top), em: em_size(h), runs, bars }
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
