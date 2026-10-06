//! Dimension geometry (port of `src/core/dim.js`). A dimension entity stores only its defining points; the drawn
//! geometry (extension lines, dimension line, arrowheads, text) is derived from the dimension style.
use crate::geom::*;
use crate::style::{fmt_num, DimStyle, Settings};
use crate::text::{measure_text, HJust, TextData, VJust};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DimType {
    Lin,
    Ali,
    Rad,
    Dia,
    Ang,
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
}

impl DimData {
    pub fn new(kind: DimType) -> DimData {
        DimData { kind, p1: None, p2: None, dp: None, c: None, pt: None, tp: None, ang: None, txt: None, ds: None }
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
                DimType::Rad => format!("R{}", b),
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
    let mk = |segs: Vec<Seg>, arrows: Vec<Arrow>, text: TextData, measured: f64, dim_line: Option<DimLine>, arc: Option<usize>| DimGeo { segs, arrows, text, measured, kind: e.kind, style: st.clone(), dim_line, arc };

    match e.kind {
        DimType::Lin | DimType::Ali => {
            let (p1, p2, dp) = (e.p1.unwrap_or(z), e.p2.unwrap_or(z), e.dp.unwrap_or(z));
            let ang = if e.kind == DimType::Ali { angle_of(p1, p2) } else { e.ang.unwrap_or(0.0) };
            let d = pt(ang.cos(), ang.sin());
            let n = perp(d);
            let side = if dot(dp - p1, n) >= 0.0 { 1.0 } else { -1.0 };
            let foot = |p: Pt| p + n * dot(dp - p, n);
            let (a, b) = (foot(p1), foot(p2));
            let measured = dot(p2 - p1, d).abs();
            let label = label_for(e, &st, measured, e.kind);
            let tw = measure_text(&label, h);
            for (p, f) in [(p1, a), (p2, b)] {
                let dir = n * side;
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
        r.pt = Some(pt(75.0 + 5.0 * 0.7071067811865476, 15.0 + 5.0 * 0.7071067811865476));
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
    fn json_matches_the_web_format() {
        let d: DimData = serde_json::from_str(r#"{"kind":"lin","p1":{"x":0,"y":0},"p2":{"x":120,"y":0},"dp":{"x":60,"y":-16},"ang":0,"ds":"Standard"}"#).unwrap();
        assert_eq!(d.kind, DimType::Lin);
        let v = serde_json::to_value(&d).unwrap();
        assert!(v.get("tp").is_none() && v["kind"] == "lin");
    }
}
