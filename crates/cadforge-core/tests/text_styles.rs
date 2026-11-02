//! Text styles survive DXF and DWG: the STYLE table (font, fixed height, width factor, obliquing angle, a TrueType
//! family in the ACAD extended data), each text's style (group 7), the current style ($TEXTSTYLE) and a dimension
//! style's text style (DIMSTYLE group 340).
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::pt;
use cadforge_core::io::{read_dwg, read_dxf, write_dwg, write_dxf};
use cadforge_core::style::TextStyle;

fn styled_drawing() -> Drawing {
    let mut d = Drawing::new("t");
    d.settings.text_styles.insert("Notes".into(), TextStyle { name: "Notes".into(), font: "Times New Roman".into(), height: 3.5, width: 0.8, oblique: 15.0 });
    d.settings.text_styles.insert("Plan".into(), TextStyle { name: "Plan".into(), font: "romans.shx".into(), height: 0.0, width: 1.0, oblique: 0.0 });
    d.settings.text_style = "Notes".into();
    d.settings.dim_styles.get_mut("Standard").unwrap().txsty = "Plan".into();
    let mut a = Entity::text(pt(0., 0.), 3.5, "TIMES");
    let mut m = Entity::mtext(pt(0., 10.), 2.5, "ROMANS", 30.0);
    for (e, s) in [(&mut a, "Notes"), (&mut m, "Plan")] {
        if let Kind::Text(t) = &mut e.kind {
            t.style = Some(s.into());
        }
    }
    d.add(a);
    d.add(m);
    d.add(Entity::text(pt(0., 20.), 2.5, "PLAIN"));
    d
}

fn check(d: &Drawing, how: &str) {
    let n = d.settings.text_style_named("Notes");
    assert_eq!((n.font.as_str(), n.height, n.width, n.oblique), ("Times New Roman", 3.5, 0.8, 15.0), "{how}");
    assert_eq!(d.settings.text_style_named("Plan").font, "romans.shx", "{how}");
    assert!(d.settings.text_style.eq_ignore_ascii_case("Notes"), "{how}: the current style");
    assert!(d.settings.dim_styles.values().any(|s| s.txsty == "Plan"), "{how}: the dimension style's text style");
    let texts: Vec<_> = d.model.entities.iter().filter_map(|e| if let Kind::Text(t) = &e.kind { Some(t.clone()) } else { None }).collect();
    let by = |s: &str| texts.iter().find(|t| t.s == s).unwrap_or_else(|| panic!("{how}: {s} is missing"));
    assert_eq!(by("TIMES").style.as_deref(), Some("Notes"), "{how}");
    assert_eq!(by("TIMES").font.as_ref().map(|f| (f.face.as_str(), f.width)), Some(("times new roman", 0.8)), "{how}: the font is applied on reading");
    assert_eq!(by("ROMANS").style.as_deref(), Some("Plan"), "{how}");
    assert!(by("PLAIN").style.is_none() && by("PLAIN").font.is_none(), "{how}");
}

#[test]
fn text_styles_round_trip_through_dxf() {
    let d = styled_drawing();
    let dxf = write_dxf(&d);
    let i = dxf.find("AcDbTextStyleTableRecord").unwrap_or(0);
    assert!(dxf.replace('\r', "").contains("1000\nTimes New Roman\n"), "the TrueType family goes in the extended data: {}", &dxf[i..(i + 400).min(dxf.len())]);
    check(&read_dxf(&dxf, "t.dxf").unwrap(), "DXF");
}

#[test]
fn text_styles_round_trip_through_dwg() {
    let bytes = write_dwg(&styled_drawing()).unwrap();
    check(&read_dwg(&bytes, "t.dwg").unwrap(), "DWG");
}

#[test]
fn json_keeps_styles_and_reapplies_fonts() {
    let d = styled_drawing();
    let back = Drawing::from_json(&d.to_json(), None).unwrap();
    check(&back, "JSON");
}
