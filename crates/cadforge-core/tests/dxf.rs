//! Ported from `tests/unit/dxf.test.js`: DXF write/read round trips and validation against ezdxf.
use cadforge_core::dim::{DimData, DimType};
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::io::dxf_read::{decode_mtext, decode_text};
use cadforge_core::io::{read_dxf, write_dxf};
use cadforge_core::sample::sample_drawing;
use cadforge_core::style::Color;
use cadforge_core::text::{HJust, VJust};
use std::process::Command;

fn near(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "expected {b}, got {a}");
}

fn full_doc() -> Drawing {
    let mut d = sample_drawing();
    d.add(Entity::arc(pt(200.0, 20.0), 15.0, 0.3, 2.2).on_layer("Outline"));
    d.add(Entity::ellipse(pt(200.0, 70.0), pt(25.0, 10.0), 0.5, 0.0, TAU).on_layer("Outline"));
    d.add(Entity::ellipse(pt(200.0, 110.0), pt(25.0, 10.0), 0.5, 0.5, 2.5).on_layer("Outline").with_color(Color::Aci(3)));
    d.add(Entity::spline(vec![pt(150.0, 0.0), pt(160.0, 20.0), pt(175.0, 5.0), pt(190.0, 25.0)], false).on_layer("Outline"));
    d.add(Entity::point(pt(120.0, 120.0)).on_layer("Notes"));
    d.add(
        Entity::pline(vec![Vertex::new(220.0, 0.0, 0.5), Vertex::new(260.0, 0.0, 0.0), Vertex::new(260.0, 30.0, -0.3), Vertex::new(220.0, 30.0, 0.0)], true)
            .on_layer("Outline")
            .with_color(Color::Rgb(0xff, 0x88, 0x00))
            .with_lw(70)
            .with_ltype("DASHED"),
    );
    let mut t = Entity::text(pt(220.0, 60.0), 5.0, "Hello \u{00D8} DXF").on_layer("Notes");
    if let Kind::Text(td) = &mut t.kind {
        td.rot = 0.4;
        td.jh = HJust::Center;
        td.jv = VJust::Middle;
    }
    d.add(t);
    d.add(Entity::xline(pt(0.0, 0.0), pt(1.0, 1.0)).on_layer("Centre"));
    let mut rad = DimData::new(DimType::Rad);
    rad.c = Some(pt(200.0, 20.0));
    rad.pt = Some(pt(215.0, 20.0));
    rad.dp = Some(pt(230.0, 40.0));
    d.add(Entity::dim(rad).on_layer("Dimensions"));
    let mut ang = DimData::new(DimType::Ang);
    ang.c = Some(pt(0.0, 0.0));
    ang.p1 = Some(pt(60.0, 0.0));
    ang.p2 = Some(pt(40.0, 40.0));
    ang.dp = Some(pt(50.0, 20.0));
    d.add(Entity::dim(ang).on_layer("Dimensions"));
    d.add(Entity::hatch(vec![vec![Vertex::new(220.0, 0.0, 0.0), Vertex::new(260.0, 0.0, 0.0), Vertex::new(260.0, 30.0, 0.0), Vertex::new(220.0, 30.0, 0.0)]], "SOLID").on_layer("Hatch").with_color(Color::Aci(5)));
    d.define_block("BOLT", pt(1.0, 2.0), vec![Entity::circle(pt(0.0, 0.0), 3.0).on_layer("0"), Entity::line(pt(-4.0, 0.0), pt(4.0, 0.0)).on_layer("0")]);
    let mut ins = Entity::insert("BOLT", pt(40.0, 60.0)).on_layer("Holes");
    if let Kind::Insert(i) = &mut ins.kind {
        i.sx = 2.0;
        i.sy = 2.0;
        i.rot = 0.5;
    }
    d.add(ins);
    d
}

fn count(d: &Drawing, ty: &str) -> usize {
    d.model.entities.iter().filter(|e| e.type_name() == ty).count()
}

#[test]
fn spline_bezier_form_reproduces_the_curve() {
    let fit = [pt(0.0, 0.0), pt(10.0, 8.0), pt(25.0, 3.0), pt(40.0, 15.0), pt(50.0, 0.0)];
    for closed in [false, true] {
        let dense = spline_points(&fit, closed, 800);
        let mut worst: f64 = 0.0;
        for b in spline_beziers(&fit, closed) {
            for k in 0..=10 {
                let (t, u) = (k as f64 / 10.0, 1.0 - k as f64 / 10.0);
                let x = u.powi(3) * b.p0.x + 3.0 * u * u * t * b.c1.x + 3.0 * u * t * t * b.c2.x + t.powi(3) * b.p1.x;
                let y = u.powi(3) * b.p0.y + 3.0 * u * u * t * b.c1.y + 3.0 * u * t * t * b.c2.y + t.powi(3) * b.p1.y;
                let mut best = f64::INFINITY;
                for w in dense.windows(2) {
                    let (a, c) = (w[0], w[1]);
                    let (dx, dy) = (c.x - a.x, c.y - a.y);
                    let l = dx * dx + dy * dy;
                    let l = if l == 0.0 { 1.0 } else { l };
                    let s = (((x - a.x) * dx + (y - a.y) * dy) / l).clamp(0.0, 1.0);
                    best = best.min((x - a.x - dx * s).hypot(y - a.y - dy * s));
                }
                worst = worst.max(best);
            }
        }
        assert!(worst < 1e-3, "deviation {worst}");
    }
}

#[test]
fn write_read_round_trip_keeps_geometry_layers_blocks_and_properties() {
    let src = full_doc();
    let back = read_dxf(&write_dxf(&src), "rt").unwrap();
    for t in ["line", "circle", "arc", "pline", "ellipse", "spline", "point", "xline", "insert", "hatch"] {
        assert!(count(&back, t) >= count(&src, t), "{t}: {} -> {}", count(&src, t), count(&back, t));
    }
    assert!(src.layers.iter().all(|l| back.layer(&l.name).is_some()), "layers survive");
    let pl = back.model.entities.iter().find(|e| matches!(&e.kind, Kind::Pline { v, closed } if v.len() == 4 && *closed)).expect("bulged polyline");
    if let Kind::Pline { v, .. } = &pl.kind {
        near(v[0].b, 0.5, 1e-6);
        near(v[2].b, -0.3, 1e-6);
    }
    assert_eq!(pl.color, Some(Color::Rgb(0xff, 0x88, 0x00)));
    assert_eq!(pl.lw, Some(70));
    assert_eq!(pl.ltype.as_deref(), Some("DASHED"));
    let circ = back.model.entities.iter().find(|e| matches!(&e.kind, Kind::Circle { c, .. } if (c.x - 75.0).abs() < 1e-9)).unwrap();
    if let Kind::Circle { r, .. } = circ.kind {
        near(r, 5.0, 1e-6);
    }
    assert_eq!(circ.layer, "Holes");
    let ell: Vec<_> = back.model.entities.iter().filter_map(|e| if let Kind::Ellipse(el) = &e.kind { Some(el.clone()) } else { None }).collect();
    assert_eq!(ell.len(), 2);
    near(ell[0].ratio, 0.5, 1e-6);
    let arc = back.model.entities.iter().find_map(|e| if let Kind::Arc { c, a0, a1, .. } = &e.kind { ((c.x - 200.0).abs() < 1e-9).then_some((*a0, *a1)) } else { None }).unwrap();
    near(arc.0, 0.3, 1e-9);
    near(arc.1, 2.2, 1e-9);
    let bolt = back.blocks.get("BOLT").expect("BOLT");
    assert_eq!(bolt.entities.len(), 2);
    near(bolt.base.x, 1.0, 1e-9);
    near(bolt.base.y, 2.0, 1e-9);
    let ins = back.model.entities.iter().find_map(|e| if let Kind::Insert(i) = &e.kind { Some(i.clone()) } else { None }).unwrap();
    near(ins.sx, 2.0, 1e-9);
    near(ins.rot, 0.5, 1e-9);
    let txt = back.model.entities.iter().find_map(|e| if let Kind::Text(t) = &e.kind { t.s.contains("Hello").then(|| t.clone()) } else { None }).unwrap();
    assert_eq!(txt.s, "Hello \u{00D8} DXF");
    assert_eq!(txt.jh, HJust::Center);
    assert_eq!(txt.jv, VJust::Middle);
    near(txt.rot, 0.4, 1e-9);
    let mt = back.model.entities.iter().find_map(|e| if let Kind::Text(t) = &e.kind { t.multi.then(|| t.clone()) } else { None }).unwrap();
    assert!(mt.s.contains("Material"));
    assert!(count(&back, "hatch") >= 2);
    assert_eq!(back.settings.units.ins_units, 4);
}

#[test]
fn dimensions_come_back_as_dimensions_or_their_block_geometry() {
    let d = sample_drawing();
    let dims = |d: &Drawing| d.model.entities.iter().filter(|e| matches!(e.kind, Kind::Dim(_))).count();
    // CadForge's own dimensions open again as dimensions
    let text = write_dxf(&d);
    assert_eq!(dims(&read_dxf(&text, "x").unwrap()), dims(&d));
    // from another program (no CadForge data) a dimension is the geometry of its block, as drawn there
    let back = read_dxf(&text.replace("CADFORGE", "OTHERAPP"), "x").unwrap();
    assert_eq!(dims(&back), 0);
    let ok = back.model.entities.iter().any(|e| if let Kind::Text(t) = &e.kind { t.s == "120" || (t.s.starts_with("120.") && t.s[4..].chars().all(|c| c == '0')) } else { false });
    assert!(ok, "linear dimension text");
    assert!(back.model.entities.iter().filter(|e| e.layer == "Dimensions").count() > 10);
}

#[test]
fn spline_control_point_import() {
    let text = write_dxf(&full_doc());
    let spline = |d: &Drawing| d.model.entities.iter().find_map(|e| if let Kind::Spline { fit, .. } = &e.kind { Some(fit.clone()) } else { None }).unwrap();
    // CadForge's spline (written as Bezier control points) opens again with its fit points
    assert_eq!(spline(&read_dxf(&text, "x").unwrap()), vec![pt(150.0, 0.0), pt(160.0, 20.0), pt(175.0, 5.0), pt(190.0, 25.0)]);
    // control points that are not CadForge's (the second one moved): the curve is sampled
    let mut lines: Vec<String> = text.split("\r\n").map(String::from).collect();
    let at = lines.iter().position(|l| l == "AcDbSpline").unwrap();
    // group codes follow the "AcDbSpline" value at every other line
    let second = (at + 1..lines.len()).step_by(2).filter(|&i| lines[i].trim() == "10").nth(1).unwrap();
    lines[second + 1] = (lines[second + 1].trim().parse::<f64>().unwrap() + 1.0).to_string();
    let moved = lines.join("\r\n");
    let fit = spline(&read_dxf(&moved, "x").unwrap());
    assert!(fit.len() > 10);
    near(fit[0].x, 150.0, 1e-6);
    near(fit.last().unwrap().x, 190.0, 1e-6);
    near(fit.last().unwrap().y, 25.0, 1e-6);
}

#[test]
fn text_decoding_helpers() {
    assert_eq!(decode_text("%%c10 %%d \\U+00D8"), "\u{2300}10 \u{00B0} \u{00D8}");
    assert_eq!(decode_mtext("{\\fArial|b0;Hello}\\Pworld \\S1/2; \\C1;red"), "Hello\nworld 1/2 red");
}

#[test]
fn reader_tolerates_minimal_r12_dxf() {
    let dxf = ["0", "SECTION", "2", "ENTITIES", "0", "LINE", "8", "A", "10", "0", "20", "0", "30", "0", "11", "10", "21", "5", "31", "0", "0", "POLYLINE", "8", "B", "66", "1", "70", "1", "0", "VERTEX", "8", "B", "10", "0", "20", "0", "42", "0.5", "0", "VERTEX", "8", "B", "10", "4", "20", "0", "0", "VERTEX", "8", "B", "10", "4", "20", "3", "0", "SEQEND", "0", "CIRCLE", "8", "A", "10", "2", "20", "2", "40", "1", "0", "ENDSEC", "0", "EOF"].join("\r\n");
    let d = read_dxf(&dxf, "min").unwrap();
    let types: Vec<_> = d.model.entities.iter().map(|e| e.type_name()).collect();
    assert_eq!(types, vec!["line", "pline", "circle"]);
    if let Kind::Pline { v, closed } = &d.model.entities[1].kind {
        assert!(closed);
        near(v[0].b, 0.5, 1e-9);
    }
    assert!(d.layer("A").is_some() && d.layer("B").is_some());
}

#[test]
fn rejects_non_dxf_input() {
    assert!(read_dxf("hello world", "x").err().unwrap().contains("DXF"));
}

fn have_ezdxf() -> bool {
    Command::new("python3").args(["-I", "-c", "import ezdxf"]).output().map(|o| o.status.success()).unwrap_or(false)
}

fn repo_file(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)
}

#[test]
fn output_validates_with_ezdxf() {
    if !have_ezdxf() {
        eprintln!("ezdxf not installed; skipping");
        return;
    }
    let file = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("out.dxf");
    std::fs::write(&file, write_dxf(&full_doc())).unwrap();
    let r = Command::new("python3").arg("-I").arg(repo_file("tests/dxf/validate.py")).arg(&file).output().unwrap();
    let out = String::from_utf8_lossy(&r.stdout).to_string() + &String::from_utf8_lossy(&r.stderr);
    assert!(r.status.success(), "{out}");
    assert!(out.contains("audit errors: []"), "{out}");
}

#[test]
fn reads_a_dxf_produced_by_ezdxf() {
    let text = std::fs::read_to_string(repo_file("tests/dxf/fixtures/ezdxf-sample.dxf")).unwrap();
    let d = read_dxf(&text, "ezdxf").unwrap();
    let mut types: std::collections::HashMap<&str, usize> = Default::default();
    for e in d.model.entities.iter().filter(|e| e.layer != "DIMS") {
        *types.entry(e.type_name()).or_default() += 1;
    }
    let t = |k: &str| types.get(k).copied().unwrap_or(0);
    assert!(t("line") >= 1 && t("circle") >= 1);
    assert_eq!(t("pline"), 1);
    assert_eq!(t("ellipse"), 1);
    assert_eq!(t("spline"), 2);
    assert_eq!(t("point"), 1);
    assert_eq!(t("insert"), 1);
    assert!(t("hatch") >= 3, "hatches {}", t("hatch"));
    assert_eq!(t("text"), 2, "text and mtext");
    let pl = d.model.entities.iter().find(|e| e.type_name() == "pline").unwrap();
    if let Kind::Pline { v, closed } = &pl.kind {
        near(v[0].b, 0.5, 1e-9);
        assert!(closed);
    }
    let tx = d.model.entities.iter().find_map(|e| if let Kind::Text(t) = &e.kind { t.s.starts_with("Hello").then(|| t.clone()) } else { None }).unwrap();
    assert_eq!(tx.s, "Hello \u{2300}25");
    assert_eq!(tx.jh, HJust::Center);
    near(tx.rot, PI / 6.0, 1e-9);
    let mt = d.model.entities.iter().find_map(|e| if let Kind::Text(t) = &e.kind { t.multi.then(|| t.clone()) } else { None }).unwrap();
    // the formatting codes are kept (the layout interprets them); read as plain text it is what it always was
    assert_eq!(mt.s, "{\\fArial|b1;Bold} line\nsecond \\S1/2;");
    assert_eq!(cadforge_core::mtext::plain(&mt.s, true), "Bold line\nsecond 1/2");
    assert!(d.model.entities.iter().any(|e| e.layer == "WALLS" && e.type_name() == "line"));
    assert_eq!(d.layer("WALLS").unwrap().color, Color::Aci(5));
    assert_eq!(d.layer("WALLS").unwrap().ltype, "DASHED");
    let edge_hatch = d.model.entities.iter().filter_map(|e| if let Kind::Hatch(h) = &e.kind { Some(h) } else { None }).find(|h| h.loops[0].iter().any(|v| v.b.abs() > 1e-6));
    assert!(edge_hatch.is_some(), "edge-path hatch with an arc edge becomes a bulged loop");
    assert!(d.model.entities.iter().filter(|e| e.layer == "DIMS").count() > 15, "dimension geometry imported from blocks");
    assert!(d.model.entities.iter().any(|e| matches!(&e.kind, Kind::Arc { c, .. } if (c.x + 200.0).abs() < 1e-9)), "mirrored arc");
    assert_eq!(d.settings.units.ins_units, 4);
}
