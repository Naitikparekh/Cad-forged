//! TOLERANCE: feature control frames, drawn as a frame, cell dividers and text in one group.
use super::{group_id, ps};
use crate::commands::helpers::pvs;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::text::{measure_text, HJust, TextData, VJust};

/// Geometric characteristics with their Unicode symbols.
const SYMBOLS: [(&str, char); 14] = [
    ("Straightness", '\u{23E4}'),
    ("Flatness", '\u{23E5}'),
    ("Circularity", '\u{25CB}'),
    ("Cylindricity", '\u{232D}'),
    ("Profile of a line", '\u{2312}'),
    ("Profile of a surface", '\u{2313}'),
    ("Angularity", '\u{2220}'),
    ("Perpendicularity", '\u{27C2}'),
    ("Parallelism", '\u{2225}'),
    ("Position", '\u{2316}'),
    ("Concentricity", '\u{25CE}'),
    ("Symmetry", '\u{232F}'),
    ("Circular runout", '\u{2197}'),
    ("Total runout", '\u{2330}'),
];

/// The symbol for a typed name: an exact name or a unique prefix of one of the characteristics (case-insensitive).
fn symbol(name: &str) -> Option<char> {
    let n = name.trim().to_lowercase();
    if let Some(s) = SYMBOLS.iter().find(|s| s.0.to_lowercase() == n) {
        return Some(s.1);
    }
    let mut hits = SYMBOLS
        .iter()
        .filter(|s| s.0.to_lowercase().starts_with(&n));
    match (hits.next(), hits.next()) {
        (Some(s), None) if !n.is_empty() => Some(s.1),
        _ => None,
    }
}

/// `%%c` / `%c` stand for the diameter symbol.
fn diameter_codes(s: &str) -> String {
    s.replace("%%c", "\u{2300}")
        .replace("%%C", "\u{2300}")
        .replace("%c", "\u{2300}")
        .replace("%C", "\u{2300}")
}

/// A feature control frame whose left edge is centred on `at`: the frame, one divider per cell boundary and the cell
/// texts (all of text height `h`).
fn frame(cells: &[String], at: Pt, h: f64) -> Vec<Entity> {
    let (ht, pad) = (2.0 * h, 0.6 * h);
    let widths: Vec<f64> = cells
        .iter()
        .map(|c| (measure_text(c, h) + 2.0 * pad).max(ht))
        .collect();
    let total: f64 = widths.iter().sum();
    let (x0, y0, y1) = (at.x, at.y - ht / 2.0, at.y + ht / 2.0);
    let mut out = vec![Entity::pline_pts(
        &[
            pt(x0, y0),
            pt(x0 + total, y0),
            pt(x0 + total, y1),
            pt(x0, y1),
        ],
        true,
    )];
    let mut x = x0;
    for (cell, w) in cells.iter().zip(&widths) {
        if x > x0 {
            out.push(Entity::line(pt(x, y0), pt(x, y1)));
        }
        out.push(Entity::new(Kind::Text(
            TextData::new(pt(x + w / 2.0, at.y), h, cell).with_just(HJust::Center, VJust::Middle),
        )));
        x += w;
    }
    out
}

async fn tolerance(c: Ctx, _a: Args) -> CmdResult<()> {
    let sym = loop {
        let name = c
            .string(ps(
                "Enter geometric characteristic symbol (name, Enter for none)",
            ))
            .await?
            .text;
        if name.is_empty() {
            break None;
        }
        match symbol(&name) {
            Some(s) => break Some(s),
            None => c.log_error(&format!(
                "Unknown symbol. Choose from: {}.",
                SYMBOLS.map(|s| s.0).join(", ")
            )),
        }
    };
    let tol = c
        .string(ps("Enter tolerance value (%%c for a diameter zone, e.g. %%c0.1 M)").raw())
        .await?
        .text;
    let mut cells: Vec<String> = sym.map(String::from).into_iter().collect();
    cells.push(diameter_codes(&tol));
    for ask in [
        "Enter primary datum (Enter for none)",
        "Enter secondary datum (Enter for none)",
        "Enter tertiary datum (Enter for none)",
    ] {
        let datum = c.string(ps(ask)).await?.text;
        if datum.is_empty() {
            break;
        }
        cells.push(datum);
    }
    let h = c.doc(|d| d.settings.cur_dim().txt * d.settings.cur_dim().scale);
    let preview_cells = cells.clone();
    let Some(at) = c
        .point(ps("Enter tolerance location").preview(move |_, p| pvs(frame(&preview_cells, p, h))))
        .await?
        .pt()
    else {
        return Ok(());
    };
    let grp = group_id(&c);
    for mut e in frame(&cells, at, h) {
        e.grp = Some(grp.clone());
        c.add(e);
    }
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(
        def("Annotation", "TOLERANCE", run(tolerance))
            .aliases(&["TOL"])
            .no_pickfirst()
            .title("Tolerance")
            .desc("Creates geometric tolerances contained in a feature control frame."),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_are_found_by_name_or_unique_prefix() {
        assert_eq!(symbol("position"), Some('\u{2316}'));
        assert_eq!(symbol("Flat"), Some('\u{23E5}'));
        assert_eq!(symbol("circular r"), Some('\u{2197}'));
        assert_eq!(symbol("circularity"), Some('\u{25CB}'));
        assert_eq!(symbol("circular"), None, "ambiguous prefixes are rejected");
        assert_eq!(symbol("profile"), None);
        assert_eq!(symbol("nonsense"), None);
    }

    #[test]
    fn the_frame_has_one_cell_per_text() {
        let cells = vec![
            "\u{2316}".to_string(),
            "\u{2300}0.1 M".to_string(),
            "A".to_string(),
        ];
        let e = frame(&cells, pt(10.0, 20.0), 2.5);
        assert_eq!(e.len(), 1 + 2 + 3, "frame, two dividers, three texts");
        let Kind::Pline { v, closed: true } = &e[0].kind else {
            panic!()
        };
        assert_eq!((v[0].y, v[2].y), (17.5, 22.5));
        assert_eq!(v[0].x, 10.0);
        assert!(
            v[1].x > 10.0 + 3.0 * 5.0,
            "the frame is as wide as its cells"
        );
    }
}
