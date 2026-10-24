//! Hatch pattern definitions (acad.pat-style line families) and line generation clipped to boundary loops
//! (port of `src/core/hatch.js`).
use crate::defs::PatternDef;
use crate::geom::*;
use std::sync::Mutex;

/// One family of parallel lines: angle `a` (deg), origin, shift `dx` along the line between successive lines,
/// perpendicular spacing `dy`, optional dash pattern (positive dash, negative gap, 0 dot).
#[derive(Clone, Copy, Debug)]
pub struct Family {
    pub a: f64,
    pub ox: f64,
    pub oy: f64,
    pub dx: f64,
    pub dy: f64,
    pub dash: Option<&'static [f64]>,
}

const fn fam(a: f64, ox: f64, oy: f64, dx: f64, dy: f64) -> Family {
    Family {
        a,
        ox,
        oy,
        dx,
        dy,
        dash: None,
    }
}
const fn famd(a: f64, ox: f64, oy: f64, dx: f64, dy: f64, dash: &'static [f64]) -> Family {
    Family {
        a,
        ox,
        oy,
        dx,
        dy,
        dash: Some(dash),
    }
}

pub struct Pattern {
    pub name: &'static str,
    pub desc: &'static str,
    pub solid: bool,
    pub families: &'static [Family],
}

pub const PATTERNS: [Pattern; 12] = [
    Pattern {
        name: "SOLID",
        desc: "Solid fill",
        solid: true,
        families: &[],
    },
    Pattern {
        name: "ANSI31",
        desc: "ANSI Iron, Brick, Stone masonry",
        solid: false,
        families: &[fam(45.0, 0.0, 0.0, 0.0, 3.175)],
    },
    Pattern {
        name: "ANSI32",
        desc: "ANSI Steel",
        solid: false,
        families: &[
            fam(45.0, 0.0, 0.0, 0.0, 9.525),
            fam(45.0, 4.49, 0.0, 0.0, 9.525),
        ],
    },
    Pattern {
        name: "ANSI33",
        desc: "ANSI Bronze, Brass, Copper",
        solid: false,
        families: &[
            fam(45.0, 0.0, 0.0, 0.0, 6.35),
            famd(45.0, 4.49, 0.0, 0.0, 6.35, &[3.175, -1.5875]),
        ],
    },
    Pattern {
        name: "ANSI37",
        desc: "ANSI Lead, Zinc, Magnesium",
        solid: false,
        families: &[
            fam(45.0, 0.0, 0.0, 0.0, 3.175),
            fam(135.0, 0.0, 0.0, 0.0, 3.175),
        ],
    },
    Pattern {
        name: "ANSI38",
        desc: "ANSI Aluminum",
        solid: false,
        families: &[
            fam(45.0, 0.0, 0.0, 0.0, 3.175),
            famd(135.0, 0.0, 0.0, 4.49, 3.175, &[7.9375, -4.7625]),
        ],
    },
    Pattern {
        name: "ANGLE",
        desc: "Angle steel",
        solid: false,
        families: &[
            famd(0.0, 0.0, 0.0, 0.0, 7.9375, &[6.35, -1.5875]),
            famd(90.0, 0.0, 0.0, 0.0, 7.9375, &[6.35, -1.5875]),
        ],
    },
    Pattern {
        name: "BRICK",
        desc: "Brick or masonry",
        solid: false,
        families: &[
            fam(0.0, 0.0, 0.0, 0.0, 6.35),
            famd(90.0, 0.0, 0.0, 6.35, 12.7, &[6.35, -6.35]),
            famd(90.0, 6.35, 6.35, 6.35, 12.7, &[6.35, -6.35]),
        ],
    },
    Pattern {
        name: "NET",
        desc: "Horizontal / vertical grid",
        solid: false,
        families: &[
            fam(0.0, 0.0, 0.0, 0.0, 3.175),
            fam(90.0, 0.0, 0.0, 0.0, 3.175),
        ],
    },
    Pattern {
        name: "DASH",
        desc: "Dashed lines",
        solid: false,
        families: &[famd(0.0, 0.0, 0.0, 3.175, 6.35, &[3.175, -3.175])],
    },
    Pattern {
        name: "LINE",
        desc: "Horizontal lines",
        solid: false,
        families: &[fam(0.0, 0.0, 0.0, 0.0, 3.175)],
    },
    Pattern {
        name: "DOTS",
        desc: "Dotted grid",
        solid: false,
        families: &[famd(0.0, 0.0, 0.0, 0.0, 3.175, &[0.0, -3.175])],
    },
];

/// The last pattern ("HONEY") is kept separate so the array above stays 12 entries wide for the UI gallery.
pub const HONEY: Pattern = Pattern {
    name: "HONEY",
    desc: "Honeycomb",
    solid: false,
    families: &[
        famd(0.0, 0.0, 0.0, 4.7625, 8.2487, &[2.7491, -5.4983]),
        famd(120.0, 0.0, 0.0, 4.7625, 8.2487, &[2.7491, -5.4983]),
        famd(60.0, 0.0, 0.0, -4.7625, 8.2487, &[2.7491, -5.4983]),
    ],
};

/// Patterns loaded from `.pat` files. A definition is leaked once so that patterns stay `&'static` like the built-in
/// ones; reloading an identical definition reuses it.
static CUSTOM: Mutex<Vec<(PatternDef, &'static Pattern)>> = Mutex::new(Vec::new());

fn custom() -> std::sync::MutexGuard<'static, Vec<(PatternDef, &'static Pattern)>> {
    CUSTOM.lock().unwrap_or_else(|e| e.into_inner())
}

fn builtin(name: &str) -> Option<&'static Pattern> {
    if name.eq_ignore_ascii_case("HONEY") {
        return Some(&HONEY);
    }
    PATTERNS.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

/// Make a pattern from a `.pat` file available by name. Built-in names cannot be replaced; loading a name again
/// replaces the earlier definition.
pub fn register_pattern(def: &PatternDef) -> Result<&'static Pattern, String> {
    let name = def.name.trim().to_uppercase();
    if name.is_empty() {
        return Err("A pattern needs a name.".into());
    }
    if def.families.is_empty() {
        return Err(format!("Pattern {name} has no line definitions."));
    }
    if builtin(&name).is_some() {
        return Err(format!(
            "{name} is a built-in pattern and cannot be replaced."
        ));
    }
    let mut reg = custom();
    let def = PatternDef {
        name: name.clone(),
        ..def.clone()
    };
    if let Some((_, p)) = reg.iter().find(|(d, _)| *d == def) {
        return Ok(p);
    }
    let families: Vec<Family> = def
        .families
        .iter()
        .map(|f| Family {
            a: f.a,
            ox: f.ox,
            oy: f.oy,
            dx: f.dx,
            dy: f.dy,
            dash: if f.dash.is_empty() {
                None
            } else {
                Some(Box::leak(f.dash.clone().into_boxed_slice()))
            },
        })
        .collect();
    let p: &'static Pattern = Box::leak(Box::new(Pattern {
        name: Box::leak(name.clone().into_boxed_str()),
        desc: Box::leak(def.desc.clone().into_boxed_str()),
        solid: false,
        families: Box::leak(families.into_boxed_slice()),
    }));
    reg.retain(|(d, _)| d.name != name);
    reg.push((def, p));
    Ok(p)
}

/// Built-in patterns, then the ones loaded from files.
pub fn all_patterns() -> Vec<&'static Pattern> {
    let mut v: Vec<&Pattern> = PATTERNS.iter().collect();
    v.push(&HONEY);
    v.extend(custom().iter().map(|(_, p)| *p));
    v
}

pub fn pattern(name: &str) -> Option<&'static Pattern> {
    builtin(name).or_else(|| {
        custom()
            .iter()
            .find(|(d, _)| d.name.eq_ignore_ascii_case(name))
            .map(|(_, p)| *p)
    })
}

pub const MAX_HATCH_LINES: usize = 60000;

/// Pattern line segments `[x0, y0, x1, y1]` clipped to the boundary polygons (even-odd filled), or `None` if too dense.
pub fn hatch_lines(
    loops: &[Vec<Pt>],
    pattern_name: &str,
    scale: f64,
    angle_deg: f64,
) -> Option<Vec<[f64; 4]>> {
    let pat = match pattern(pattern_name) {
        Some(p) => p,
        None => return Some(vec![]),
    };
    if pat.solid || loops.is_empty() {
        return Some(vec![]);
    }
    let mut out: Vec<[f64; 4]> = vec![];
    let ua = rad(angle_deg);
    for f in pat.families {
        let th = rad(f.a) + ua;
        let (ux, uy) = (th.cos(), th.sin());
        let (vx, vy) = (-uy, ux);
        let step = f.dy * scale;
        if step.abs() < 1e-9 {
            continue;
        }
        let (ca, sa) = (ua.cos(), ua.sin());
        let o = pt(
            (f.ox * ca - f.oy * sa) * scale,
            (f.ox * sa + f.oy * ca) * scale,
        );
        let (ou, os) = (o.x * ux + o.y * uy, o.x * vx + o.y * vy);
        let mut edges: Vec<[f64; 4]> = vec![];
        let (mut smin, mut smax) = (f64::INFINITY, f64::NEG_INFINITY);
        for l in loops {
            for i in 0..l.len() {
                let (a, b) = (l[i], l[(i + 1) % l.len()]);
                let (au, a_s, bu, b_s) = (
                    a.x * ux + a.y * uy,
                    a.x * vx + a.y * vy,
                    b.x * ux + b.y * uy,
                    b.x * vx + b.y * vy,
                );
                edges.push([au, a_s, bu, b_s]);
                smin = smin.min(a_s).min(b_s);
                smax = smax.max(a_s).max(b_s);
            }
        }
        let (q0, q1) = ((smin - os) / step, (smax - os) / step);
        let k0 = (q0.min(q1) - 1e-9).ceil() as i64;
        let k1 = (q0.max(q1) + 1e-9).floor() as i64;
        if (k1 - k0) as usize > MAX_HATCH_LINES {
            return None;
        }
        let dash: Option<Vec<f64>> = f.dash.map(|d| d.iter().map(|x| x * scale).collect());
        let period: f64 = dash
            .as_ref()
            .map(|d| d.iter().map(|x| x.abs()).sum())
            .unwrap_or(0.0);
        for k in k0..=k1 {
            let s0 = os + k as f64 * step;
            let mut us: Vec<f64> = vec![];
            for e in &edges {
                let (au, a_s, bu, b_s) = (e[0], e[1], e[2], e[3]);
                if (a_s < s0) != (b_s < s0) {
                    us.push(au + (s0 - a_s) / (b_s - a_s) * (bu - au));
                }
            }
            us.sort_by(|p, q| p.partial_cmp(q).unwrap());
            let mut i = 0;
            while i + 1 < us.len() {
                let (u0, u1) = (us[i], us[i + 1]);
                i += 2;
                if u1 - u0 < 1e-9 {
                    continue;
                }
                match &dash {
                    Some(d) if period >= 1e-9 => {
                        let base = ou + k as f64 * f.dx * scale;
                        let mut pos = base + ((u0 - base) / period).floor() * period;
                        while pos < u1 {
                            for dd in d {
                                let len = dd.abs();
                                if *dd >= 0.0 {
                                    let (a, b) = (pos.max(u0), (pos + len).min(u1));
                                    if b >= a - 1e-12 && pos + len >= u0 && pos <= u1 {
                                        out.push([
                                            a * ux + s0 * vx,
                                            a * uy + s0 * vy,
                                            b * ux + s0 * vx,
                                            b * uy + s0 * vy,
                                        ]);
                                    }
                                }
                                pos += len;
                                if pos >= u1 {
                                    break;
                                }
                            }
                            if out.len() > MAX_HATCH_LINES * 4 {
                                return None;
                            }
                        }
                    }
                    _ => out.push([
                        u0 * ux + s0 * vx,
                        u0 * uy + s0 * vy,
                        u1 * ux + s0 * vx,
                        u1 * uy + s0 * vy,
                    ]),
                }
            }
            if out.len() > MAX_HATCH_LINES * 4 {
                return None;
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Vec<Pt> {
        vec![pt(x, y), pt(x + s, y), pt(x + s, y + s), pt(x, y + s)]
    }

    #[test]
    fn ansi31_spacing_and_clipping() {
        let lines = hatch_lines(&[square(0.0, 0.0, 10.0)], "ANSI31", 1.0, 0.0).unwrap();
        // 45° lines every 3.175 perpendicular: spacing along x is 3.175*sqrt(2) ≈ 4.49 -> about 4-5 lines cross a 10x10 square, each clipped to it
        assert!(lines.len() >= 5 && lines.len() <= 8, "{}", lines.len());
        for l in &lines {
            for (x, y) in [(l[0], l[1]), (l[2], l[3])] {
                assert!(x >= -1e-9 && x <= 10.0 + 1e-9 && y >= -1e-9 && y <= 10.0 + 1e-9);
            }
            let (dx, dy) = (l[2] - l[0], l[3] - l[1]);
            assert!((dx.abs() - dy.abs()).abs() < 1e-9, "45 degrees");
        }
    }

    #[test]
    fn holes_use_even_odd_and_scale_changes_density() {
        let outer = square(0.0, 0.0, 20.0);
        let hole = square(5.0, 5.0, 10.0);
        let full = hatch_lines(&[outer.clone()], "LINE", 1.0, 0.0).unwrap();
        let holed = hatch_lines(&[outer.clone(), hole], "LINE", 1.0, 0.0).unwrap();
        assert!(
            holed.len() > full.len(),
            "lines through the hole are split in two"
        );
        let total = |v: &Vec<[f64; 4]>| {
            v.iter()
                .map(|l| (l[2] - l[0]).hypot(l[3] - l[1]))
                .sum::<f64>()
        };
        assert!(total(&holed) < total(&full));
        let sparse = hatch_lines(&[outer], "LINE", 2.0, 0.0).unwrap();
        assert!(sparse.len() < full.len());
    }

    #[test]
    fn dashes_solid_and_density_limit() {
        let dashed = hatch_lines(&[square(0.0, 0.0, 20.0)], "DASH", 1.0, 0.0).unwrap();
        assert!(dashed.len() > 5);
        assert!(hatch_lines(&[square(0.0, 0.0, 20.0)], "SOLID", 1.0, 0.0)
            .unwrap()
            .is_empty());
        assert!(hatch_lines(&[square(0.0, 0.0, 1e6)], "LINE", 0.001, 0.0).is_none());
        assert!(all_patterns().len() == 13 && pattern("honey").is_some());
    }
}
