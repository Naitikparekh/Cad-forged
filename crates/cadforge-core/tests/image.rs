//! Raster images: attaching, persistence, DXF round trip (checked by ezdxf) and adjustments.
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::io::{read_dxf, write_dxf};
use cadforge_core::picture::{encode_png, Decoded};
use std::process::Command;

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

/// A 4 x 2 test picture: left half red, right half blue, bottom-right pixel transparent.
fn test_png() -> Vec<u8> {
    let mut px = vec![];
    for y in 0..2 {
        for x in 0..4 {
            let a = if x == 3 && y == 1 { 0 } else { 255 };
            px.extend(if x < 2 { [255, 0, 0, a] } else { [0, 0, 255, a] });
        }
    }
    encode_png(4, 2, px).unwrap()
}

fn doc_with_image() -> (Drawing, String) {
    let mut d = Drawing::new("pics");
    let key = d.add_image("C:\\site\\Plan view.png", test_png()).unwrap();
    d.add(Entity::new(Kind::Image(ImageData::new(&key, pt(10.0, 20.0), 40.0, 20.0, 0.0))));
    (d, key)
}

#[test]
fn attaching_reads_size_and_makes_a_unique_name() {
    let mut d = Drawing::new("t");
    let k1 = d.add_image("/tmp/photo.png", test_png()).unwrap();
    let k2 = d.add_image("photo.png", test_png()).unwrap();
    assert_eq!(k1, "photo");
    assert_eq!(k2, "photo2");
    let def = &d.images["photo"];
    assert_eq!((def.w, def.h), (4, 2));
    let px = def.decoded().unwrap();
    assert_eq!(&px.rgba[0..4], &[255, 0, 0, 255]);
    assert_eq!(&px.rgba[(4 * 1 + 3) * 4..(4 * 1 + 3) * 4 + 4], &[0, 0, 255, 0]);
    assert!(d.add_image("x.png", b"not an image".to_vec()).is_err());
    assert!(d.add_image("x.gif", b"GIF89a....".to_vec()).is_err());
}

#[test]
fn pictures_survive_the_project_file() {
    let (d, key) = doc_with_image();
    let back = Drawing::from_json_str(&d.to_json_string(), None).unwrap();
    assert_eq!(back.images[&key], d.images[&key], "the encoded file travels inside the project");
    assert_eq!(back.images[&key].decoded().unwrap().w, 4);
    assert!(matches!(&back.model.entities[0].kind, Kind::Image(i) if i.name == key));
}

#[test]
fn geometry_follows_transformations() {
    let (mut d, _) = doc_with_image();
    let id = d.model.entities[0].id;
    // rotate 90 degrees about the origin, then mirror: u and v are transformed as vectors
    d.modify(id, |e| transform_entity(e, &mat::rotate(std::f64::consts::FRAC_PI_2, Pt::ZERO)));
    let Kind::Image(i) = &d.by_id(id).unwrap().kind else { panic!() };
    near(i.u.x, 0.0);
    near(i.u.y, 40.0);
    near(i.v.x, -20.0);
    near(i.p.x, -20.0);
    near(i.p.y, 10.0);
    near(i.rotation(), std::f64::consts::FRAC_PI_2);
    let b = bounds_of(d.by_id(id).unwrap(), &d).unwrap();
    near(b.min_x, -40.0);
    near(b.max_x, -20.0);
}

#[test]
fn a_clip_is_cut_to_the_picture_and_picked_inside() {
    let (mut d, _) = doc_with_image();
    let id = d.model.entities[0].id;
    d.modify(id, |e| {
        if let Kind::Image(i) = &mut e.kind {
            i.clip = vec![pt(0.0, 0.0), pt(30.0, 0.0), pt(30.0, 30.0), pt(0.0, 30.0)]; // spills over the left and bottom
        }
    });
    let e = d.by_id(id).unwrap();
    let Kind::Image(i) = &e.kind else { panic!() };
    let b = i.boundary();
    assert!(b.iter().all(|q| q.x >= 10.0 - 1e-9 && q.y >= 20.0 - 1e-9 && q.x <= 30.0 + 1e-9 && q.y <= 30.0 + 1e-9), "{b:?}");
    assert!(pick_distance(e, pt(15.0, 25.0), &d) == 0.0, "inside the visible part");
    assert!(pick_distance(e, pt(45.0, 35.0), &d) > 1.0, "outside the clip");
}

#[test]
fn adjustments_change_pixels_but_neutral_does_not() {
    let px = Decoded { w: 1, h: 1, rgba: vec![100, 150, 200, 255] };
    assert_eq!(px.adjusted(50, 50, 0, [255, 255, 255]), px.rgba);
    let bright = px.adjusted(75, 50, 0, [255, 255, 255]);
    assert!(bright[0] > 100 && bright[1] > 150);
    let faded = px.adjusted(50, 50, 100, [255, 255, 255]);
    assert_eq!(&faded[..3], &[255, 255, 255]);
    let flat = px.adjusted(50, 0, 0, [255, 255, 255]);
    assert_eq!(&flat[..3], &[128, 128, 128]);
    assert_eq!(flat[3], 255, "alpha is never adjusted");
}

#[test]
fn dxf_round_trip_keeps_placement_clip_and_adjustments() {
    let (mut d, key) = doc_with_image();
    let id = d.model.entities[0].id;
    d.modify(id, |e| {
        if let Kind::Image(i) = &mut e.kind {
            i.clip = vec![pt(15.0, 22.0), pt(40.0, 25.0), pt(20.0, 38.0)];
            i.brightness = 60;
            i.contrast = 40;
            i.fade = 10;
        }
    });
    let dxf = write_dxf(&d);
    assert!(dxf.contains("IMAGEDEF") && dxf.contains("AcDbRasterImage"));
    let back = read_dxf(&dxf, "rt").unwrap();
    let Kind::Image(i) = &back.model.entities[0].kind else { panic!("{:?}", back.model.entities) };
    near(i.p.x, 10.0);
    near(i.p.y, 20.0);
    near(i.u.x, 40.0);
    near(i.v.y, 20.0);
    assert_eq!((i.brightness, i.contrast, i.fade), (60, 40, 10));
    assert_eq!(i.clip.len(), 3);
    near(i.clip[0].x, 15.0);
    near(i.clip[0].y, 22.0);
    near(i.clip[2].x, 20.0);
    // the DXF names the file but does not carry it
    let def = &back.images[&i.name];
    assert!(def.is_missing(), "a DXF refers to the picture file, it does not embed it");
    assert_eq!((def.w, def.h), (4, 2));
    assert_eq!(def.path, d.images[&key].path);
}

#[test]
fn ezdxf_accepts_the_image_and_agrees_on_the_clip() {
    if !Command::new("python3").args(["-I", "-c", "import ezdxf"]).output().map(|o| o.status.success()).unwrap_or(false) {
        eprintln!("ezdxf not installed; skipping");
        return;
    }
    let (mut d, _) = doc_with_image();
    let id = d.model.entities[0].id;
    d.modify(id, |e| {
        if let Kind::Image(i) = &mut e.kind {
            i.clip = vec![pt(15.0, 22.0), pt(40.0, 25.0), pt(20.0, 38.0)];
        }
    });
    let out = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("image-rt.dxf");
    std::fs::write(&out, write_dxf(&d)).unwrap();
    let script = r#"
import sys, ezdxf
from ezdxf import recover
doc, aud = recover.readfile(sys.argv[1])
assert not aud.has_errors, [e.message for e in aud.errors][:5]
im = doc.modelspace().query('IMAGE').first
assert im is not None, 'no IMAGE entity'
assert im.image_def is not None and im.image_def.dxf.filename.endswith('Plan view.png'), im.image_def
assert tuple(im.dxf.image_size)[:2] == (4, 2), im.dxf.image_size
assert im.dxf.insert.isclose((10, 20, 0)), im.dxf.insert
print(sorted((round(v.x, 3), round(v.y, 3)) for v in im.boundary_path_wcs()))
"#;
    let r = Command::new("python3").args(["-I", "-c", script, out.to_str().unwrap()]).output().unwrap();
    assert!(r.status.success(), "{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
    let printed = String::from_utf8_lossy(&r.stdout).to_string();
    // ezdxf's reading of the boundary path must match the polygon we meant
    for (x, y) in [("15.0", "22.0"), ("40.0", "25.0"), ("20.0", "38.0")] {
        assert!(printed.contains(&format!("({x}, {y})")), "{printed}");
    }
}
