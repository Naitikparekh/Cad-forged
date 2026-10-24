//! DXF output of the arc-length, jogged and ordinate dimension types.
use cadforge_core::dim::{DimData, DimType};
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::io::{read_dxf, write_dxf};
use std::process::Command;

fn doc() -> Drawing {
    let mut d = Drawing::new("dims");
    let mut arc = DimData::new(DimType::Arc);
    arc.c = Some(pt(0.0, 0.0));
    arc.p1 = Some(pt(10.0, 0.0));
    arc.p2 = Some(pt(0.0, 10.0));
    arc.dp = Some(pt(14.0, 14.0));
    let mut jog = DimData::new(DimType::Jog);
    jog.c = Some(pt(50.0, 0.0));
    jog.p1 = Some(pt(30.0, 0.0));
    jog.pt = Some(pt(60.0, 0.0));
    jog.p2 = Some(pt(45.0, 0.0));
    jog.dp = Some(pt(52.0, 3.0));
    let mut ord = DimData::new(DimType::Ord);
    ord.c = Some(pt(0.0, 0.0));
    ord.pt = Some(pt(30.0, 20.0));
    ord.dp = Some(pt(30.0, 40.0));
    ord.xd = true;
    for k in [arc, jog, ord] {
        d.add(Entity::dim(k));
    }
    d
}

#[test]
fn new_dimension_types_survive_a_dxf_round_trip() {
    let text = write_dxf(&doc());
    assert!(text.contains("AcDbOrdinateDimension") && text.contains("AcDbRadialDimension") && text.contains("AcDb3PointAngularDimension"));
    // CadForge opens them again as the same dimensions
    let back = read_dxf(&text, "dims").unwrap();
    let kinds: Vec<DimType> = back.model.entities.iter().filter_map(|e| if let Kind::Dim(d) = &e.kind { Some(d.kind) } else { None }).collect();
    assert_eq!(kinds, vec![DimType::Arc, DimType::Jog, DimType::Ord]);
    // other programs draw their blocks: without CadForge's extended data they read as that geometry
    let plain = text.replace("CADFORGE", "OTHERAPP");
    let geo = read_dxf(&plain, "dims").unwrap();
    assert!(geo.model.entities.iter().any(|e| matches!(&e.kind, Kind::Text(t) if t.s == "30.00")), "the ordinate text");
    assert!(geo.model.entities.iter().any(|e| matches!(&e.kind, Kind::Text(t) if t.s.starts_with('R'))), "the jogged radius text");
    assert!(geo.model.entities.iter().any(|e| e.type_name() == "arc"), "the arc-length dimension arc");
}

#[test]
fn new_dimension_types_validate_with_ezdxf() {
    if !Command::new("python3").args(["-I", "-c", "import ezdxf"]).output().map(|o| o.status.success()).unwrap_or(false) {
        eprintln!("ezdxf not installed; skipping");
        return;
    }
    let file = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("dim_ext.dxf");
    std::fs::write(&file, write_dxf(&doc())).unwrap();
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/dxf/validate.py");
    let r = Command::new("python3").arg("-I").arg(script).arg(&file).output().unwrap();
    let out = String::from_utf8_lossy(&r.stdout).to_string() + &String::from_utf8_lossy(&r.stderr);
    assert!(r.status.success() && out.contains("audit errors: []"), "{out}");
}
