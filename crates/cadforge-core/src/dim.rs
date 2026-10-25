//! Dimension geometry (port of `src/core/dim.js`). A dimension entity stores only its defining points; the drawn
//! geometry (extension lines, dimension line, arrowheads, text) is derived from the dimension style.
use crate::geom::*;
use crate::style::{fmt_num, DimStyle, Settings};
use crate::text::{measure_text, text_corners, HJust, TextData, VJust};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DimType {
    Lin,
    Ali,
    Rad,
    Dia,
    Ang,
    /// Arc length (DIMARC).
    Arc,
    /// Jogged radius (DIMJOGGED): `c` true centre, `p1` centre override, `pt` point on the arc, `p2` jog location.
    Jog,
    /// Ordinate (DIMORDINATE): `c` datum origin, `pt` feature location, `dp` leader end.
    Ord,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DimData {
    pub kind: DimType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p1: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p2: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dp: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pt: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tp: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ang: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub txt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ds: Option<String>,
    /// Ordinate dimensions: measures the X coordinate (otherwise Y).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub xd: bool,
    /// Text rotation added to the automatic text angle (DIMTEDIT / DIMEDIT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tr: Option<f64>,
    /// Extension-line obliquing: deviation from perpendicular (DIMEDIT Oblique).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obl: Option<f64>,
    /// Breaks (DIMBREAK): `(segment index, from, to)`, distances along a line segment of the unbroken geometry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub brk: Vec<(usize, f64, f64)>,
    /// Inspection frame (DIMINSPECT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insp: Option<DimInspect>,
    /// Links of the defining points to the geometry they measure (an associative dimension follows it).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assoc: Vec<DimRef>,
}

/// One defining point of an associative dimension, tied to a point of another object (see `dim_assoc.rs`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DimRef {
    /// The defining point: `p1`, `p2`, `c` or `pt`.
    pub key: String,
    /// The object it lies on.
    pub ent: u64,
    /// Which of the object's snap points (its index in `snap_points_of`) and of what kind.
    #[serde(default)]
    pub i: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<crate::entities::SnapType>,
    /// The point stays on the circle or arc, at the same angle from its centre (radius and diameter dimensions).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub radial: bool,
}

/// Inspection dimension data: label, inspection rate and frame shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DimInspect {
    #[serde(default)]
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<u32>,
    #[serde(default)]
    pub shape: InspShape,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InspShape {
    #[default]
    Round,
    Angular,
    None,
}

impl DimData {
    pub fn new(kind: DimType) -> DimData {
        DimData { kind, p1: None, p2: None, dp: None, c: None, pt: None, tp: None, ang: None, txt: None, ds: None, xd: false, tr: None, obl: None, brk: vec![], insp: None, assoc: vec![] }
    }
    /// Names of the defining-point fields of this dimension.
    pub fn point_mut(&mut self, key: &str) -> Option<&mut Option<Pt>> {
        Some(match key {
            "p1" => &mut self.p1,
            "p2" => &mut self.p2,
            "dp" => &mut self.dp,
            "c" => &mut self.c,
            "pt" => &mut self.pt,
            "tp" => &mut self.tp,
            _ => return None,
        })
    }
    pub fn point(&self, key: &str) -> Option<Pt> {
        match key {
            "p1" => self.p1,
            "p2" => self.p2,
            "dp" => self.dp,
            "c" => self.c,
            "pt" => self.pt,
            "tp" => self.tp,
            _ => None,
        }
    }
}

pub const DIM_POINT_KEYS: [&str; 6] = ["p1", "p2", "dp", "c", "pt", "tp"];

#[derive(Clone, Debug)]
pub struct Arrow {
    pub tip: Pt,
    pub dir: Pt,
    pub size: f64,
    pub style: String,
}

#[derive(Clone, Debug)]
pub struct DimLine {
    pub a: Pt,
    pub b: Pt,
    pub dir: Pt,
    pub normal: Pt,
    pub side: f64,
}

#[derive(Clone, Debug)]
pub struct DimGeo {
    pub segs: Vec<Seg>,
    pub arrows: Vec<Arrow>,
    pub text: TextData,
    pub measured: f64,
    pub kind: DimType,
    pub style: DimStyle,
    pub dim_line: Option<DimLine>,
    /// Index into `segs` of the arc of an angular dimension.
    pub arc: Option<usize>,
}

fn readable(a: f64) -> f64 {
    let a = norm_angle_signed(a);
    if a > PI / 2.0 + 1e-9 {
        a - PI
    } else if a <= -PI / 2.0 + 1e-9 {
        a + PI
    } else {
        a
    }
}

pub fn dim_style_of<'a>(e: &DimData, settings: &'a Settings) -> &'a DimStyle {
    e.ds.as_ref().and_then(|n| settings.dim_styles.get(n)).unwrap_or_else(|| settings.cur_dim())
}

fn measure_label(value: f64, st: &DimStyle) -> String {
    let mut s = fmt_num(value * st.lfac, st.dec.max(0) as usize, false);
    if st.zin & 8 != 0 && s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    s
}

fn label_for(e: &DimData, st: &DimStyle, measured: f64, kind: DimType) -> String {
    let base = match kind {
        DimType::Ang => {
            let s = fmt_num(measured * 180.0 / PI, 0, false);
            format!("{}°", s)
        }
        _ => {
            let b = measure_label(measured, st);
            match kind {
                DimType::Rad | DimType::Jog => format!("R{}", b),
                DimType::Dia => format!("\u{2300}{}", b),
                _ => b,
            }
        }
    };
    let base = format!("{}{}{}", st.pre, base, st.suf);
    match &e.txt {
        None => base,
        Some(t) if t.is_empty() => base,
        Some(t) if t.contains("<>") => t.replace("<>", &base),
        Some(t) => t.clone(),
    }
}

/// Derive drawable geometry from a dimension entity.
pub fn dim_geometry(e: &DimData, settings: &Settings) -> DimGeo {
    let st = dim_style_of(e, settings).clone();
    let k = st.scale;
    let (asz, exo, exe, gap, h) = (st.asz * k, st.exo * k, st.exe * k, st.gap * k, st.txt * k);
    let mut segs: Vec<Seg> = vec![];
    let mut arrows: Vec<Arrow> = vec![];
    let z = Pt::ZERO;
    macro_rules! line {
        ($a:expr, $b:expr) => {
            segs.push(Seg::line($a, $b))
        };
    }
    macro_rules! arrow {
        ($tip:expr, $dir:expr) => {
            arrows.push(Arrow { tip: $tip, dir: $dir, size: asz, style: st.blk.clone() })
        };
    }
    let mk = |mut segs: Vec<Seg>, arrows: Vec<Arrow>, mut text: TextData, measured: f64, dim_line: Option<DimLine>, arc: Option<usize>| {
        if let Some(r) = e.tr {
            text.rot = norm_angle(text.rot + r);
        }
        if arc.is_none() && !e.brk.is_empty() {
            segs = apply_breaks(segs, &e.brk);
        }
        if e.kind == DimType::Arc {
            segs.push(arc_symbol(&text, gap));
        }
        if let Some(i) = &e.insp {
            inspection_frame(i, &mut text, &mut segs, gap);
        }
        DimGeo { segs, arrows, text, measured, kind: e.kind, style: st.clone(), dim_line, arc }
    };

    match e.kind {
        DimType::Lin | DimType::Ali => {
            let (p1, p2, dp) = (e.p1.unwrap_or(z), e.p2.unwrap_or(z), e.dp.unwrap_or(z));
            let ang = if e.kind == DimType::Ali { angle_of(p1, p2) } else { e.ang.unwrap_or(0.0) };
            let d = pt(ang.cos(), ang.sin());
            let n = perp(d);
            let side = if dot(dp - p1, n) >= 0.0 { 1.0 } else { -1.0 };
            // extension lines run along `ev`: perpendicular to the dimension line unless obliqued
            let obl = e.obl.unwrap_or(0.0);
            let ev = pt(n.x * obl.cos() - n.y * obl.sin(), n.x * obl.sin() + n.y * obl.cos());
            let cos_obl = dot(ev, n).max(0.05);
            let foot = |p: Pt| p + ev * (dot(dp - p, n) / cos_obl);
            let (a, b) = (foot(p1), foot(p2));
            let measured = dot(p2 - p1, d).abs();
            let label = label_for(e, &st, measured, e.kind);
            let tw = measure_text(&label, h);
            for (p, f) in [(p1, a), (p2, b)] {
                let dir = ev * side;
                let far = f + dir * exe;
                line!(if dist(p, f) < exo { f } else { p + dir * exo }, far);
            }
            let l = dist(a, b);
            let line_dir = if l > 1e-12 { unit(b - a) } else { d };
            let arrows_fit = l >= asz * 2.2;
            let text_fits = l >= tw + asz * 2.0 + gap * 2.0;
            if arrows_fit {
                arrow!(a, line_dir);
                arrow!(b, line_dir * -1.0);
                line!(a, b);
            } else {
                arrow!(a, line_dir * -1.0);
                arrow!(b, line_dir);
                line!(a + line_dir * (-asz * 2.0), b + line_dir * (asz * 2.0));
            }
            let text_ang = readable(line_dir.y.atan2(line_dir.x));
            let n_txt = pt(-text_ang.sin(), text_ang.cos());
            let mp = mid(a, b);
            let mut jv = VJust::Middle;
            let tp = if let Some(t) = e.tp {
                t
            } else if text_fits || !arrows_fit {
                if st.tad {
                    jv = VJust::Bottom;
                    mp + n_txt * gap
                } else {
                    mp
                }
            } else {
                let t = b + line_dir * (asz * 2.0 + gap + tw / 2.0);
                if st.tad {
                    t + n_txt * (gap + h / 2.0)
                } else {
                    t
                }
            };
            let text = TextData::new(tp, h, &label).with_rot(text_ang).with_just(HJust::Center, jv);
            mk(segs, arrows, text, measured, Some(DimLine { a, b, dir: line_dir, normal: n, side }), None)
        }
        DimType::Rad | DimType::Dia => {
            let (c, p) = (e.c.unwrap_or(z), e.pt.unwrap_or(z));
            let dp = e.dp.unwrap_or(p);
            let r = dist(c, p);
            let u = unit(p - c);
            let measured = if e.kind == DimType::Rad { r } else { 2.0 * r };
            let label = label_for(e, &st, measured, e.kind);
            let far = if e.kind == DimType::Dia { c - u * r } else { c };
            if e.kind == DimType::Dia {
                arrow!(far, u);
            }
            arrow!(p, u * -1.0);
            line!(far, p);
            let text = if dist(c, dp) < r {
                TextData::new(e.tp.unwrap_or_else(|| mid(c, p)), h, &label).with_rot(readable(angle_of(c, p))).with_just(HJust::Center, if st.tad { VJust::Bottom } else { VJust::Middle })
            } else {
                line!(p, dp);
                let right = dp.x >= p.x;
                let sgn = if right { 1.0 } else { -1.0 };
                line!(dp, dp + pt(sgn * asz, 0.0));
                TextData::new(dp + pt(sgn * (asz + gap), 0.0), h, &label).with_just(if right { HJust::Left } else { HJust::Right }, VJust::Middle)
            };
            if st.cen > 0.0 {
                let cm = st.cen * k / 2.0;
                line!(pt(c.x - cm, c.y), pt(c.x + cm, c.y));
                line!(pt(c.x, c.y - cm), pt(c.x, c.y + cm));
            }
            mk(segs, arrows, text, measured, None, None)
        }
        DimType::Ang => {
            let (c, p1, p2, dp) = (e.c.unwrap_or(z), e.p1.unwrap_or(z), e.p2.unwrap_or(z), e.dp.unwrap_or(z));
            let (a1, a2) = (angle_of(c, p1), angle_of(c, p2));
            let rr = dist(c, dp);
            let (mut a0, mut sw) = (a1, sweep_ccw(a1, a2));
            if sweep_ccw(a1, angle_of(c, dp)) > sw {
                a0 = a2;
                sw = TAU - sw;
            }
            let arc = Seg::arc(c, rr, a0, sw);
            segs.push(arc);
            let arc_idx = segs.len() - 1;
            for (p, ang) in [(p1, a1), (p2, a2)] {
                let d = pt(ang.cos(), ang.sin());
                let tip_pt = polar(c, ang, rr);
                let from = if dist(c, p) > exo { p + d * exo } else { c };
                if dist(c, from) < rr + exe {
                    line!(from, tip_pt + d * exe);
                }
            }
            arrow!(polar(c, a0, rr), pt(a0.sin(), -a0.cos()));
            arrow!(polar(c, a0 + sw, rr), pt(-(a0 + sw).sin(), (a0 + sw).cos()));
            let am = a0 + sw / 2.0;
            let label = label_for(e, &st, sw, DimType::Ang);
            let text = TextData::new(e.tp.unwrap_or_else(|| polar(c, am, rr + gap)), h, &label).with_rot(readable(am + PI / 2.0)).with_just(HJust::Center, VJust::Bottom);
            mk(segs, arrows, text, sw, None, Some(arc_idx))
        }
        DimType::Arc => {
            let (c, p1, p2, dp) = (e.c.unwrap_or(z), e.p1.unwrap_or(z), e.p2.unwrap_or(z), e.dp.unwrap_or(z));
            let (a0, r) = (angle_of(c, p1), dist(c, p1));
            let sw = match sweep_ccw(a0, angle_of(c, p2)) {
                s if s < 1e-12 => TAU,
                s => s,
            };
            let rr = dist(c, dp).max(1e-9);
            segs.push(Seg::arc(c, rr, a0, sw));
            let arc_idx = segs.len() - 1;
            let outward = if rr >= r { 1.0 } else { -1.0 };
            for ang in [a0, a0 + sw] {
                line!(polar(c, ang, r + outward * exo), polar(c, ang, rr + outward * exe));
            }
            arrow!(polar(c, a0, rr), pt(a0.sin(), -a0.cos()));
            arrow!(polar(c, a0 + sw, rr), pt(-(a0 + sw).sin(), (a0 + sw).cos()));
            let am = a0 + sw / 2.0;
            let label = label_for(e, &st, r * sw, DimType::Arc);
            let text = TextData::new(e.tp.unwrap_or_else(|| polar(c, am, rr + gap)), h, &label).with_rot(readable(am + PI / 2.0)).with_just(HJust::Center, VJust::Bottom);
            mk(segs, arrows, text, r * sw, None, Some(arc_idx))
        }
        DimType::Jog => {
            let (c, p) = (e.c.unwrap_or(z), e.pt.unwrap_or(z));
            let (o, dp) = (e.p1.unwrap_or(c), e.dp.unwrap_or(p));
            let l = dist(o, p);
            let w = unit(o - p);
            let n = perp(w);
            let measured = dist(c, p);
            let label = label_for(e, &st, measured, DimType::Jog);
            // the dimension line from the arc point to the centre override carries a zig-zag at the jog location
            let s = asz * 1.5;
            let jc = p + w * clamp(dot(e.p2.unwrap_or(mid(o, p)) - p, w), l * 0.2, l * 0.8);
            let zig = [p, jc - w * (s / 2.0), jc - w * (s / 6.0) + n * (s / 2.0), jc + w * (s / 6.0) - n * (s / 2.0), jc + w * (s / 2.0), o];
            for pair in zig.windows(2) {
                line!(pair[0], pair[1]);
            }
            arrow!(p, w);
            let text = if dot(dp - p, w) < 0.0 {
                // text beyond the arc: continue the line and finish with a short landing
                line!(p, dp);
                let right = dp.x >= p.x;
                let sgn = if right { 1.0 } else { -1.0 };
                line!(dp, dp + pt(sgn * asz, 0.0));
                TextData::new(dp + pt(sgn * (asz + gap), 0.0), h, &label).with_just(if right { HJust::Left } else { HJust::Right }, VJust::Middle)
            } else {
                TextData::new(e.tp.unwrap_or(dp), h, &label).with_rot(readable(angle_of(o, p))).with_just(HJust::Center, if st.tad { VJust::Bottom } else { VJust::Middle })
            };
            mk(segs, arrows, text, measured, None, None)
        }
        DimType::Ord => {
            let (o, f, end) = (e.c.unwrap_or(z), e.pt.unwrap_or(z), e.dp.unwrap_or(z));
            let sgn = |v: f64| if v < 0.0 { -1.0 } else { 1.0 };
            let (measured, tip, dir) = if e.xd { (f.x - o.x, pt(f.x, end.y), pt(0.0, sgn(end.y - f.y))) } else { (f.y - o.y, pt(end.x, f.y), pt(sgn(end.x - f.x), 0.0)) };
            let label = label_for(e, &st, measured, DimType::Ord);
            if dist(f, tip) > exo {
                line!(f + dir * exo, tip);
            }
            let (rot, jh) = if e.xd { (PI / 2.0, if dir.y > 0.0 { HJust::Left } else { HJust::Right }) } else { (0.0, if dir.x > 0.0 { HJust::Left } else { HJust::Right }) };
            let text = TextData::new(e.tp.unwrap_or(tip + dir * gap), h, &label).with_rot(rot).with_just(jh, VJust::Middle);
            mk(segs, arrows, text, measured, None, None)
        }
    }
}

/// Cut the breaks `(segment index, from, to)` (distances along a line segment) out of `segs`.
fn apply_breaks(segs: Vec<Seg>, brk: &[(usize, f64, f64)]) -> Vec<Seg> {
    let mut out = Vec::with_capacity(segs.len() + brk.len());
    for (i, s) in segs.into_iter().enumerate() {
        let mut cuts: Vec<(f64, f64)> = brk.iter().filter(|b| b.0 == i).map(|b| (b.1.min(b.2), b.1.max(b.2))).collect();
        let Seg::L { a, b, .. } = s else {
            out.push(s);
            continue;
        };
        let (l, d) = (dist(a, b), unit(b - a));
        cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut at = 0.0;
        for (c0, c1) in cuts {
            let (c0, c1) = (c0.clamp(0.0, l), c1.clamp(0.0, l));
            if c0 > at + 1e-9 {
                out.push(Seg::line(a + d * at, a + d * c0));
            }
            at = at.max(c1);
        }
        if at < l - 1e-9 {
            out.push(Seg::line(a + d * at, b));
        }
    }
    out
}

/// The arc symbol of an arc-length dimension, drawn above its text.
fn arc_symbol(text: &TextData, gap: f64) -> Seg {
    let k = text_corners(text);
    let (top, up, chord) = (mid(k[2], k[3]), unit(k[3] - k[0]), dist(k[2], k[3]));
    let s = (chord / 4.0).min(text.h / 2.0);
    let r = (chord * chord / 4.0 + s * s) / (2.0 * s);
    let phi = (chord / 2.0 / r).asin();
    Seg::arc(top + up * (gap / 2.0 + s - r), r, angle_of(Pt::ZERO, up) - phi, 2.0 * phi)
}

/// DIMINSPECT: "label | value | rate%" text inside a round or angular frame.
fn inspection_frame(i: &DimInspect, text: &mut TextData, segs: &mut Vec<Seg>, gap: f64) {
    let mut parts: Vec<String> = vec![];
    if !i.label.is_empty() {
        parts.push(i.label.clone());
    }
    parts.push(text.s.clone());
    if let Some(r) = i.rate {
        parts.push(format!("{r}%"));
    }
    text.s = parts.join(" | ");
    if i.shape == InspShape::None {
        return;
    }
    let k = text_corners(text);
    let (u, v) = (unit(k[1] - k[0]), unit(k[3] - k[0]));
    let (ctr, hh) = (mid(k[0], k[2]), dist(k[0], k[3]) / 2.0 + gap / 2.0);
    let hw = dist(k[0], k[1]) / 2.0 + gap + if i.shape == InspShape::Round { hh / 2.0 } else { 0.0 };
    let straight = if i.shape == InspShape::Round { hw - hh } else { hw };
    for s in [-1.0, 1.0] {
        segs.push(Seg::line(ctr - u * straight + v * (s * hh), ctr + u * straight + v * (s * hh)));
    }
    if i.shape == InspShape::Round {
        let rot = angle_of(Pt::ZERO, u);
        segs.push(Seg::arc(ctr + u * straight, hh, rot - PI / 2.0, PI));
        segs.push(Seg::arc(ctr - u * straight, hh, rot + PI / 2.0, PI));
    } else {
        for s in [-1.0, 1.0] {
            segs.push(Seg::line(ctr + u * (s * hw) - v * hh, ctr + u * (s * hw) + v * hh));
        }
    }
}

/// Closed-triangle arrowhead polygon (tip, then two base corners). `None` for non-filled styles.
pub fn arrow_polygon(a: &Arrow) -> Option<[Pt; 3]> {
    if a.style != "closed" {
        return None;
    }
    let n = pt(-a.dir.y, a.dir.x);
    let base = a.tip + a.dir * a.size;
    Some([a.tip, base + n * (a.size / 6.0), base + n * (-a.size / 6.0)])
}

/// Open/tick arrowhead strokes as point pairs.
pub fn arrow_lines(a: &Arrow) -> Vec<(Pt, Pt)> {
    let n = pt(-a.dir.y, a.dir.x);
    match a.style.as_str() {
        "open" => {
            let base = a.tip + a.dir * a.size;
            vec![(a.tip, base + n * (a.size / 4.0)), (a.tip, base + n * (-a.size / 4.0))]
        }
        "tick" => {
            let q = unit(a.dir + n) * (a.size / 2.0);
            vec![(a.tip - q, a.tip + q)]
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lin(p1: Pt, p2: Pt, dp: Pt, ang: f64) -> DimData {
        let mut d = DimData::new(DimType::Lin);
        d.p1 = Some(p1);
        d.p2 = Some(p2);
        d.dp = Some(dp);
        d.ang = Some(ang);
        d
    }

    #[test]
    fn linear_dimension_geometry() {
        let s = Settings::default();
        let g = dim_geometry(&lin(pt(0.0, 0.0), pt(120.0, 0.0), pt(60.0, -16.0), 0.0), &s);
        assert!((g.measured - 120.0).abs() < 1e-12);
        assert_eq!(g.text.s, "120.00");
        assert_eq!(g.arrows.len(), 2);
        assert_eq!(g.segs.len(), 3);                       // two extension lines + dimension line
        let dl = g.dim_line.unwrap();
        assert!((dl.a.y + 16.0).abs() < 1e-12 && (dl.b.x - 120.0).abs() < 1e-12 && dl.side < 0.0);
        assert!((g.text.p.x - 60.0).abs() < 1e-9, "text centred on the dimension line");
        assert!((g.text.p.y + 15.375).abs() < 1e-9 && g.text.jv == VJust::Bottom, "text sits above the dimension line (DIMTAD)");
        // extension lines start at the offset from the origin and extend beyond the line
        match g.segs[0] {
            Seg::L { a, b, .. } => {
                assert!((a.y + 0.625).abs() < 1e-9 && (b.y + 17.25).abs() < 1e-9);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn text_override_and_vertical_text_reads_upwards() {
        let s = Settings::default();
        let mut d = lin(pt(0.0, 0.0), pt(0.0, 90.0), pt(-16.0, 45.0), PI / 2.0);
        let g = dim_geometry(&d, &s);
        assert_eq!(g.text.s, "90.00");
        assert!((g.text.rot - PI / 2.0).abs() < 1e-9);
        d.txt = Some("<> THRU".into());
        assert_eq!(dim_geometry(&d, &s).text.s, "90.00 THRU");
        d.txt = Some("EXACT".into());
        assert_eq!(dim_geometry(&d, &s).text.s, "EXACT");
    }

    #[test]
    fn short_dimensions_flip_the_arrows_outside() {
        let s = Settings::default();
        let g = dim_geometry(&lin(pt(0.0, 0.0), pt(4.0, 0.0), pt(2.0, -10.0), 0.0), &s);
        // arrows point inward from outside: tip of the first arrow at a, direction away from b
        assert!(g.arrows[0].dir.x < 0.0 && g.arrows[1].dir.x > 0.0);
    }

    #[test]
    fn radius_diameter_and_angular() {
        let s = Settings::default();
        let mut r = DimData::new(DimType::Dia);
        r.c = Some(pt(75.0, 15.0));
        r.pt = Some(pt(75.0 + 5.0 * std::f64::consts::FRAC_1_SQRT_2, 15.0 + 5.0 * std::f64::consts::FRAC_1_SQRT_2));
        r.dp = Some(pt(94.0, 29.0));
        let g = dim_geometry(&r, &s);
        assert_eq!(g.text.s, "\u{2300}10.00");
        assert_eq!(g.arrows.len(), 2);
        assert_eq!(g.text.jh, HJust::Left);
        r.kind = DimType::Rad;
        let g = dim_geometry(&r, &s);
        assert_eq!(g.text.s, "R5.00");
        let mut a = DimData::new(DimType::Ang);
        a.c = Some(pt(0.0, 0.0));
        a.p1 = Some(pt(60.0, 0.0));
        a.p2 = Some(pt(0.0, 60.0));
        a.dp = Some(pt(40.0, 40.0));
        let g = dim_geometry(&a, &s);
        assert_eq!(g.text.s, "90°");
        assert!(g.arc.is_some());
        assert!((g.measured - PI / 2.0).abs() < 1e-9);
        assert_eq!(arrow_polygon(&g.arrows[0]).unwrap().len(), 3);
        let mut open = g.arrows[0].clone();
        open.style = "open".into();
        assert_eq!(arrow_lines(&open).len(), 2);
        assert!(arrow_polygon(&open).is_none());
    }

    #[test]
    fn arc_length_measures_radius_times_sweep() {
        let s = Settings::default();
        let mut d = DimData::new(DimType::Arc);
        d.c = Some(pt(0.0, 0.0));
        d.p1 = Some(pt(10.0, 0.0));
        d.p2 = Some(pt(0.0, 10.0));
        d.dp = Some(pt(14.0, 14.0));
        let g = dim_geometry(&d, &s);
        assert!((g.measured - 10.0 * PI / 2.0).abs() < 1e-9);
        assert_eq!(g.text.s, "15.71");
        let r = dist(pt(0.0, 0.0), pt(14.0, 14.0));
        assert!(matches!(g.segs[g.arc.unwrap()], Seg::A { r: rr, .. } if (rr - r).abs() < 1e-9));
        assert_eq!(g.arrows.len(), 2);
        // two extension lines, the dimension arc and the arc symbol above the text
        assert_eq!(g.segs.len(), 4);
        let Seg::A { c, r: sr, a0, sw, .. } = g.segs[3] else { panic!("arc symbol") };
        let up = polar(Pt::ZERO, g.text.rot + PI / 2.0, 1.0);
        let top = g.text.p + up * g.text.h;
        assert!(dot(polar(c, a0 + sw / 2.0, sr) - top, up) > 0.0, "the symbol sits above the text");
        let width = dist(polar(c, a0, sr), polar(c, a0 + sw, sr));
        assert!((width - measure_text(&g.text.s, g.text.h)).abs() < 1e-9, "as wide as the text");
    }

    #[test]
    fn jogged_and_ordinate_dimensions() {
        let s = Settings::default();
        let mut j = DimData::new(DimType::Jog);
        j.c = Some(pt(0.0, 0.0));
        j.p1 = Some(pt(-20.0, 0.0));
        j.pt = Some(pt(10.0, 0.0));
        j.p2 = Some(pt(-5.0, 0.0));
        j.dp = Some(pt(0.0, 3.0));
        let g = dim_geometry(&j, &s);
        assert_eq!(g.text.s, "R10.00");
        assert_eq!(g.segs.len(), 5, "line, three zig-zag strokes, line");
        assert_eq!(g.arrows.len(), 1);
        let mut o = DimData::new(DimType::Ord);
        o.c = Some(pt(0.0, 0.0));
        o.pt = Some(pt(30.0, 20.0));
        o.dp = Some(pt(30.0, 40.0));
        o.xd = true;
        let g = dim_geometry(&o, &s);
        assert_eq!((g.measured, g.text.s.as_str()), (30.0, "30.00"));
        assert!((g.text.rot - PI / 2.0).abs() < 1e-12 && g.text.p.y > 40.0);
        o.xd = false;
        o.dp = Some(pt(60.0, 20.0));
        assert_eq!(dim_geometry(&o, &s).text.s, "20.00");
    }

    #[test]
    fn breaks_text_rotation_and_oblique_extension_lines() {
        let s = Settings::default();
        let mut d = lin(pt(0.0, 0.0), pt(100.0, 0.0), pt(50.0, -10.0), 0.0);
        let plain = dim_geometry(&d, &s);
        assert_eq!(plain.segs.len(), 3);
        d.brk = vec![(2, 40.0, 45.0)];
        let g = dim_geometry(&d, &s);
        assert_eq!(g.segs.len(), 4, "the dimension line is cut in two");
        assert!((g.segs[2].length() - 40.0).abs() < 1e-9 && (g.segs[3].length() - 55.0).abs() < 1e-9);
        d.brk.clear();
        d.tr = Some(PI / 2.0);
        assert!((dim_geometry(&d, &s).text.rot - PI / 2.0).abs() < 1e-9);
        d.tr = None;
        d.obl = Some(PI / 4.0);
        let g = dim_geometry(&d, &s);
        let Seg::L { a, b, .. } = g.segs[0] else { panic!() };
        let r = 0.5f64.sqrt();
        assert!((a.x - 0.625 * r).abs() < 1e-9 && (b.x - (10.0 + 1.25 * r)).abs() < 1e-9, "the extension line leans: {a:?} {b:?}");
        // the dimension line moves along the leaning extension line; the measurement is unchanged
        let dl = g.dim_line.unwrap();
        assert!((dl.a.x - 10.0).abs() < 1e-9 && (dl.a.y + 10.0).abs() < 1e-9 && (g.measured - 100.0).abs() < 1e-9);
    }

    #[test]
    fn inspection_dimensions_frame_the_text() {
        let s = Settings::default();
        let mut d = lin(pt(0.0, 0.0), pt(100.0, 0.0), pt(50.0, -10.0), 0.0);
        let base = dim_geometry(&d, &s).segs.len();
        d.insp = Some(DimInspect { label: "A1".into(), rate: Some(50), shape: InspShape::Round });
        let g = dim_geometry(&d, &s);
        assert_eq!(g.text.s, "A1 | 100.00 | 50%");
        assert_eq!(g.segs.len(), base + 4, "two lines and two end arcs");
        d.insp = Some(DimInspect { label: String::new(), rate: None, shape: InspShape::Angular });
        assert_eq!(dim_geometry(&d, &s).segs.len(), base + 4);
    }

    #[test]
    fn new_fields_keep_saved_json_compatible() {
        let d = lin(pt(0.0, 0.0), pt(1.0, 0.0), pt(0.5, 1.0), 0.0);
        let v = serde_json::to_value(&d).unwrap();
        for k in ["xd", "tr", "obl", "brk", "insp"] {
            assert!(v.get(k).is_none(), "{k} is omitted when unset");
        }
        let back: DimData = serde_json::from_value(v).unwrap();
        assert_eq!(back, d);
        let mut broken = d.clone();
        (broken.brk, broken.tr, broken.obl, broken.insp) = (vec![(2, 40.0, 45.0)], Some(0.5), Some(-0.2), Some(DimInspect { label: "A".into(), rate: Some(50), shape: InspShape::Angular }));
        let json = serde_json::to_string(&broken).unwrap();
        assert!(json.contains(r#""brk":[[2,40.0,45.0]]"#) && json.contains(r#""shape":"angular""#), "{json}");
        assert_eq!(serde_json::from_str::<DimData>(&json).unwrap(), broken);
        let o: DimData = serde_json::from_str(r#"{"kind":"ord","c":{"x":0,"y":0},"pt":{"x":3,"y":4},"dp":{"x":3,"y":9},"xd":true}"#).unwrap();
        assert!(o.xd && o.kind == DimType::Ord);
    }

    #[test]
    fn json_matches_the_web_format() {
        let d: DimData = serde_json::from_str(r#"{"kind":"lin","p1":{"x":0,"y":0},"p2":{"x":120,"y":0},"dp":{"x":60,"y":-16},"ang":0,"ds":"Standard"}"#).unwrap();
        assert_eq!(d.kind, DimType::Lin);
        let v = serde_json::to_value(&d).unwrap();
        assert!(v.get("tp").is_none() && v["kind"] == "lin");
    }
}
