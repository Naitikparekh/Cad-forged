//! User-supplied definition files: hatch patterns (`.pat`, the acad.pat layout) and linetypes (`.lin`, the acad.lin
//! layout). They are parsed into owned definitions, which a drawing keeps (so they are saved with it) and registers
//! with [`crate::hatch::register_pattern`] / [`crate::style::register_linetype`].
use serde::{Deserialize, Serialize};

/// One line family of a hatch pattern: angle (deg), origin, shift along the line, spacing between lines and an optional
/// dash pattern (positive dash, negative gap, 0 dot).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FamilyDef {
    pub a: f64,
    pub ox: f64,
    pub oy: f64,
    pub dx: f64,
    pub dy: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dash: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternDef {
    pub name: String,
    #[serde(default)]
    pub desc: String,
    pub families: Vec<FamilyDef>,
}

/// A simple linetype: positive = dash, negative = gap, 0 = dot, in drawing units at LTSCALE 1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinetypeDef {
    pub name: String,
    #[serde(default)]
    pub desc: String,
    pub pattern: Vec<f64>,
}

/// What a file held: the definitions that could be used and a message for each one that could not.
#[derive(Debug)]
pub struct Parsed<T> {
    pub defs: Vec<T>,
    pub skipped: Vec<String>,
}

fn clean(line: &str) -> &str {
    line.split(';').next().unwrap_or("").trim()
}

/// `*NAME` or `*NAME, description` -> (upper-case name, description).
fn header(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix('*')?;
    let (name, desc) = match rest.split_once(',') {
        Some((n, d)) => (n.trim(), d.trim()),
        None => (rest.trim(), ""),
    };
    Some((name.to_uppercase(), desc.to_string()))
}

fn numbers(line: &str) -> Result<Vec<f64>, String> {
    line.split(',').map(|t| t.trim().parse::<f64>().map_err(|_| format!("\"{}\" is not a number", t.trim()))).collect()
}

/// Parse the text of a `.pat` file.
pub fn parse_pat(text: &str) -> Parsed<PatternDef> {
    let mut out = Parsed { defs: vec![], skipped: vec![] };
    let mut cur: Option<(PatternDef, Option<String>)> = None;
    let finish = |cur: &mut Option<(PatternDef, Option<String>)>, out: &mut Parsed<PatternDef>| {
        if let Some((def, err)) = cur.take() {
            match err {
                Some(e) => out.skipped.push(format!("{}: {e}", def.name)),
                None if def.families.is_empty() => out.skipped.push(format!("{}: no line definitions", def.name)),
                None => out.defs.push(def),
            }
        }
    };
    for raw in text.lines() {
        let line = clean(raw);
        if line.is_empty() {
            continue;
        }
        if let Some((name, desc)) = header(line) {
            finish(&mut cur, &mut out);
            cur = Some((PatternDef { name, desc, families: vec![] }, None));
        } else if let Some((def, err)) = cur.as_mut() {
            if err.is_some() {
                continue;
            }
            match numbers(line) {
                Ok(n) if n.len() >= 5 => {
                    let (a, ox, oy, dx, dy) = (n[0], n[1], n[2], n[3], n[4]);
                    if dy == 0.0 && dx == 0.0 {
                        *err = Some("a line family needs a spacing".into());
                    } else {
                        def.families.push(FamilyDef { a, ox, oy, dx, dy, dash: n[5..].to_vec() });
                    }
                }
                Ok(_) => *err = Some("a line family needs angle, origin, delta-x and delta-y".into()),
                Err(e) => *err = Some(e),
            }
        }
    }
    finish(&mut cur, &mut out);
    out
}

/// Parse the text of a `.lin` file. Linetypes with embedded shapes or text are skipped.
pub fn parse_lin(text: &str) -> Parsed<LinetypeDef> {
    let mut out = Parsed { defs: vec![], skipped: vec![] };
    let mut lines = text.lines().map(clean).filter(|l| !l.is_empty()).peekable();
    while let Some(line) = lines.next() {
        let Some((name, desc)) = header(line) else { continue };
        let Some(spec) = lines.next_if(|l| !l.starts_with('*')) else {
            out.skipped.push(format!("{name}: no pattern line"));
            continue;
        };
        let Some(body) = spec.strip_prefix(['A', 'a']) else {
            out.skipped.push(format!("{name}: the pattern line must start with A"));
            continue;
        };
        if body.contains('[') {
            out.skipped.push(format!("{name}: shapes and text in linetypes are not supported"));
            continue;
        }
        match numbers(body.trim_start_matches(',')) {
            Ok(n) if n.iter().any(|v| *v != 0.0) && n.iter().map(|v| v.abs()).sum::<f64>() > 0.0 => out.defs.push(LinetypeDef { name, desc, pattern: n }),
            Ok(_) => out.skipped.push(format!("{name}: the pattern has no length")),
            Err(e) => out.skipped.push(format!("{name}: {e}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pat_file_gives_families_with_and_without_dashes() {
        let p = parse_pat("; comment\n*BOX, Boxes  ; trailing\n0, 0,0, 0,6.35\n90, 0,0, 0,6.35, 3,-3\n\n*BAD,x\n45, 1,2\n*EMPTY\n");
        assert_eq!(p.defs.len(), 1);
        assert_eq!(p.defs[0].name, "BOX");
        assert_eq!(p.defs[0].desc, "Boxes");
        assert_eq!(p.defs[0].families.len(), 2);
        assert_eq!(p.defs[0].families[1].dash, vec![3.0, -3.0]);
        assert_eq!(p.skipped.len(), 2, "{:?}", p.skipped);
    }

    #[test]
    fn a_lin_file_gives_simple_linetypes_and_skips_complex_ones() {
        let l = parse_lin("*FENCE, fence line\nA,12,-6,0,-6\n*GAS_LINE, Gas ----GAS----\nA,12.7,-5.08,[\"GAS\",STANDARD,S=2.5,U=0.0,X=-2.54,Y=-1.27],-5.08\n*BAD\nA,x\n");
        assert_eq!(l.defs, vec![LinetypeDef { name: "FENCE".into(), desc: "fence line".into(), pattern: vec![12.0, -6.0, 0.0, -6.0] }]);
        assert_eq!(l.skipped.len(), 2, "{:?}", l.skipped);
        assert!(l.skipped[0].contains("GAS_LINE"));
    }
}
