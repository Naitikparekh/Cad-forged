//! DXF files other CAD programs open. AutoCAD (and DWG TrueView) discards the whole drawing for one fault, so the
//! writer must never produce: a line longer than 2049 characters (long MTEXT goes in 250-character group 3 pieces),
//! TEXT over 255 characters, a line break inside a value (written as ^J), an LWPOLYLINE with both a constant width
//! and vertex widths, undeclared object classes, or an object AutoCAD refuses (radius or text height 0, a coordinate
//! that is not a number). Such objects are left out and Save says so.
use cadforge_core::dim::{DimData, DimType};
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::{pt, Vertex};
use cadforge_core::io::{not_kept_in_dxf, read_dwg, read_dxf, write_dwg, write_dxf};

fn long_note() -> String {
    let mut s = String::new();
    for i in 0..60 {
        s.push_str(&format!("Note {i}: bend radius 2.5° café, tolerance ±0.1 mm; "));
        if i % 10 == 9 {
            s.push('\n');
        }
    }
    s.trim_end().to_string()
}

fn drawing() -> Drawing {
    let mut d = Drawing::new("strict");
    d.add_layer("Pièces").unwrap();
    d.add(Entity::mtext(pt(0., 0.), 2.5, &long_note(), 200.0).on_layer("Pièces"));
    d.add(Entity::text(pt(0., -20.), 2.5, &"X".repeat(400)));
    let mut dim = DimData::new(DimType::Lin);
    (dim.p1, dim.p2, dim.dp) = (Some(pt(0., -40.)), Some(pt(50., -40.)), Some(pt(25., -50.)));
    dim.txt = Some("2 PLACES\n^ <>".into());
    d.add(Entity::dim(dim));
    let mut v = vec![Vertex::new(0., -60., 0.), Vertex::new(20., -60., 0.), Vertex::new(40., -60., 0.)];
    (v[1].sw, v[1].ew) = (1.0, 2.0);
    d.add(Entity::pline(v, false));
    // objects AutoCAD refuses
    d.add(Entity::circle(pt(5., 5.), 0.0));
    d.add(Entity::line(pt(f64::NAN, 0.), pt(1., 1.)));
    d.add(Entity::text(pt(0., 9.), 0.0, "flat"));
    d
}

/// The (code, value) pairs of a DXF file, checking its line structure on the way.
fn pairs(dxf: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = dxf.split("\r\n").collect();
    let lines = if lines.last() == Some(&"") { &lines[..lines.len() - 1] } else { &lines[..] };
    assert!(lines.len() % 2 == 0, "pairs of lines");
    lines.chunks(2).map(|p| (p[0].trim().parse::<i32>().unwrap_or_else(|_| panic!("a group code: {:?}", p[0])), p[1].to_string())).collect()
}

#[test]
fn the_file_has_nothing_autocad_refuses() {
    let d = drawing();
    let dxf = write_dxf(&d);
    assert!(!dxf.replace("\r\n", "").contains(['\r', '\n']), "no stray line breaks");
    let g = pairs(&dxf);
    for (c, v) in &g {
        assert!(v.len() <= 2049, "group {c}: a line of {} characters", v.len());
        if (10..60).contains(c) {
            assert!(v.parse::<f64>().is_ok_and(f64::is_finite), "group {c} = {v}");
        }
    }
    // the MTEXT: 250-character group 3 pieces, then the rest in group 1
    let at = g.iter().position(|(c, v)| *c == 0 && v == "MTEXT").unwrap();
    let end = at + 1 + g[at + 1..].iter().position(|(c, _)| *c == 0).unwrap();
    let body: Vec<&(i32, String)> = g[at..end].iter().filter(|(c, _)| *c == 3 || *c == 1).collect();
    assert!(body.len() > 4 && body.last().unwrap().0 == 1 && body[..body.len() - 1].iter().all(|(c, v)| *c == 3 && v.len() <= 250), "{body:?}");
    // TEXT holds at most 255 characters
    assert!(g.iter().any(|(c, v)| *c == 1 && v.len() == 255 && v.chars().all(|ch| ch == 'X')));
    // the polyline with widths on one vertex: no constant width, widths on every vertex
    let at = g.iter().position(|(c, v)| *c == 0 && v == "LWPOLYLINE").unwrap();
    let end = at + 1 + g[at + 1..].iter().position(|(c, _)| *c == 0).unwrap();
    let pl = &g[at..end];
    assert!(!pl.iter().any(|(c, _)| *c == 43) && pl.iter().filter(|(c, _)| *c == 40).count() == 3, "{pl:?}");
    // the classes AutoCAD needs declared
    for cls in ["LAYOUT", "IMAGE", "WIPEOUT"] {
        assert!(g.windows(2).any(|w| w[0] == (0, "CLASS".into()) && w[1] == (1, cls.into())), "class {cls}");
    }
    // the broken objects are left out, and Save says so
    assert_eq!(g.iter().filter(|(c, v)| *c == 0 && v == "CIRCLE").count(), 0);
    assert!(!g.iter().any(|(c, v)| *c == 1 && v == "flat"));
    let note = not_kept_in_dxf(&d, false).join("; ");
    assert!(note.contains("3 broken objects are left out"), "{note}");
}

#[test]
fn text_comes_back_exactly_from_dxf_and_dwg() {
    let d = drawing();
    let check = |back: &Drawing, how: &str| {
        let texts: Vec<String> = back.model.entities.iter().filter_map(|e| if let Kind::Text(t) = &e.kind { Some(t.s.clone()) } else { None }).collect();
        assert!(texts.contains(&long_note()), "{how}: the long note comes back whole");
        let dim = back.model.entities.iter().find_map(|e| if let Kind::Dim(d) = &e.kind { d.txt.clone() } else { None });
        assert_eq!(dim.as_deref(), Some("2 PLACES\n^ <>"), "{how}: the line break and the caret come back");
        assert!(back.layers.iter().any(|l| l.name == "Pièces"), "{how}");
        assert_eq!(back.model.entities.len(), 4, "{how}: the four good objects");
    };
    check(&read_dxf(&write_dxf(&d), "s.dxf").unwrap(), "DXF");
    check(&read_dwg(&write_dwg(&d).unwrap(), "s.dwg").unwrap(), "DWG");
}
