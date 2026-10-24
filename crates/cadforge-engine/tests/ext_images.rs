//! IMAGEATTACH, IMAGE, IMAGECLIP, IMAGEFRAME, IMAGEADJUST and IMAGEQUALITY.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::picture::encode_png;
use cadforge_engine::app::Event;
use cadforge_engine::runtime::{ArgVal, Args};
use cadforge_engine::sim::Sim;

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

fn png(w: u32, h: u32) -> Vec<u8> {
    encode_png(w, h, vec![200; (w * h * 4) as usize]).unwrap()
}

fn with_picture() -> (Sim, String) {
    let s = Sim::new();
    let key = s.doc_mut(|d| d.add_image("plan.png", png(100, 50)).unwrap());
    (s, key)
}

fn image_entity(s: &Sim) -> (u64, ImageData) {
    s.ents()
        .into_iter()
        .find_map(|e| {
            if let Kind::Image(i) = e.kind {
                Some((e.id, i))
            } else {
                None
            }
        })
        .expect("an image")
}

fn attach(s: &Sim, key: &str) {
    s.cmd_args(
        "IMAGEATTACH",
        Args::none().with("name", ArgVal::Str(key.into())),
    );
}

#[test]
fn imageattach_places_scales_and_rotates_the_picture() {
    let (s, key) = with_picture();
    attach(&s, &key);
    assert_eq!(s.prompt(), "Specify insertion point <0,0>:");
    s.click(10., 20.);
    assert!(
        s.prompt().starts_with("Specify scale factor"),
        "{}",
        s.prompt()
    );
    s.ty("0.5");
    assert!(s.prompt().starts_with("Specify rotation angle"));
    s.ty("90");
    assert!(!s.busy());
    let (_, i) = image_entity(&s);
    near(i.p.x, 10.0);
    near(i.p.y, 20.0);
    near(i.size().0, 50.0);
    near(i.size().1, 25.0);
    near(i.rotation(), std::f64::consts::FRAC_PI_2);
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
}

#[test]
fn imageattach_unit_option_takes_the_width_in_drawing_units() {
    let (s, key) = with_picture();
    attach(&s, &key);
    s.click(0., 0.).ty("Unit").ty("200").enter();
    let (_, i) = image_entity(&s);
    near(i.size().0, 200.0);
    near(i.size().1, 100.0);
}

#[test]
fn imageattach_without_a_file_asks_the_host() {
    let s = Sim::new();
    s.h.borrow_mut().ui.dialogs = true;
    s.cmd("IMAGEATTACH");
    let asked =
        s.h.borrow_mut()
            .take_events()
            .into_iter()
            .any(|e| matches!(e, Event::UiAction { name, .. } if name == "IMAGEATTACH"));
    assert!(asked);
    let headless = Sim::new();
    headless.cmd("IMAGEATTACH");
    assert!(headless.history_has("IMAGEATTACH needs a picture"));
}

#[test]
fn imageclip_rectangle_is_aligned_with_the_picture_and_can_be_toggled() {
    let (s, key) = with_picture();
    attach(&s, &key);
    s.click(0., 0.).ty("1").ty("0");
    // rotate the picture 90 degrees afterwards so that "aligned with the picture" differs from "aligned with the screen"
    let (id, _) = image_entity(&s);
    s.doc_mut(|d| {
        d.modify(id, |e| {
            transform_entity(e, &mat::rotate(std::f64::consts::FRAC_PI_2, Pt::ZERO))
        })
    });
    // the picture now spans x in [-50, 0], y in [0, 100]
    s.cmd("IMAGECLIP").click(-25., 50.);
    assert!(s.prompt().starts_with("Enter image clipping option"));
    s.enter().enter(); // New boundary, Rectangular
    s.click(-10., 10.).click(-30., 40.);
    let (_, i) = image_entity(&s);
    assert_eq!(i.clip.len(), 4);
    let (xs, ys): (Vec<f64>, Vec<f64>) = (
        i.clip.iter().map(|q| q.x).collect(),
        i.clip.iter().map(|q| q.y).collect(),
    );
    let ext = |v: &[f64]| {
        (
            v.iter().cloned().fold(f64::MAX, f64::min),
            v.iter().cloned().fold(f64::MIN, f64::max),
        )
    };
    assert_eq!(
        (
            ext(&xs).0.round(),
            ext(&xs).1.round(),
            ext(&ys).0.round(),
            ext(&ys).1.round()
        ),
        (-30.0, -10.0, 10.0, 40.0)
    );
    s.cmd("IMAGECLIP").click(-20., 20.).ty("OFF");
    assert!(image_entity(&s).1.clip_off);
    s.cmd("IMAGECLIP").click(-20., 20.).ty("ON");
    assert!(!image_entity(&s).1.clip_off);
    s.cmd("IMAGECLIP").click(-20., 20.).ty("Delete");
    assert!(image_entity(&s).1.clip.is_empty());
}

#[test]
fn imageclip_polygon_needs_three_points() {
    let (s, key) = with_picture();
    attach(&s, &key);
    s.click(0., 0.).ty("1").ty("0");
    s.cmd("IMAGECLIP").click(50., 25.).enter().ty("Polygonal");
    s.click(10., 10.).click(60., 10.).enter();
    assert!(s.history_has("at least three points"));
    assert!(image_entity(&s).1.clip.is_empty());
    s.cmd("IMAGECLIP").click(50., 25.).enter().ty("Polygonal");
    s.click(10., 10.).click(60., 10.).click(30., 40.).enter();
    assert_eq!(image_entity(&s).1.clip.len(), 3);
}

#[test]
fn imageadjust_sets_brightness_contrast_and_fade() {
    let (s, key) = with_picture();
    attach(&s, &key);
    s.click(0., 0.).ty("1").ty("0");
    s.cmd("IMAGEADJUST")
        .click(50., 25.)
        .enter()
        .ty("70")
        .ty("30")
        .ty("15");
    let (_, i) = image_entity(&s);
    assert_eq!((i.brightness, i.contrast, i.fade), (70, 30, 15));
    s.cmd("IMAGEADJUST")
        .click(50., 25.)
        .enter()
        .ty("500")
        .enter()
        .enter();
    let (_, i) = image_entity(&s);
    assert_eq!(
        (i.brightness, i.fade),
        (100, 15),
        "values are clamped; Enter keeps the current one"
    );
}

#[test]
fn imageframe_and_imagequality_are_drawing_settings() {
    let s = Sim::new();
    s.cmd("IMAGEFRAME").ty("2");
    assert_eq!(s.doc(|d| d.settings.image_frame), 2);
    s.cmd("IMAGEFRAME").ty("3");
    assert!(s.history_has("Enter 0, 1 or 2"));
    assert_eq!(s.doc(|d| d.settings.image_frame), 2);
    s.cmd("IMAGEQUALITY").ty("Draft");
    assert!(s.doc(|d| d.settings.image_draft));
    s.cmd("IMAGEQUALITY").ty("High");
    assert!(!s.doc(|d| d.settings.image_draft));
}

#[test]
fn image_lists_and_detaches() {
    let (s, key) = with_picture();
    s.cmd("IMAGE").enter();
    assert!(s.history_has("plan.png  100 x 50 px  0 references"));
    attach(&s, &key);
    s.click(0., 0.).ty("1").ty("0");
    s.cmd("IMAGE").enter();
    assert!(s.history_has("1 reference"));
    s.cmd("IMAGE").ty("Detach").ty("nothing");
    assert!(s.history_has("not found"));
    s.cmd("IMAGE").ty("Detach").ty(&key);
    assert_eq!(s.n(), 0, "the references go with the picture");
    assert!(s.doc(|d| d.images.is_empty()));
}

#[test]
fn objects_move_copy_and_grip_like_any_entity() {
    let (s, key) = with_picture();
    attach(&s, &key);
    s.click(0., 0.).ty("1").ty("0");
    let (id, _) = image_entity(&s);
    s.select_ids(&[id])
        .cmd("MOVE")
        .click(0., 0.)
        .click(30., 30.);
    near(image_entity(&s).1.p.x, 30.0);
    s.cmd("COPY")
        .click(50., 55.)
        .enter()
        .click(0., 0.)
        .click(200., 0.)
        .enter();
    assert_eq!(s.n(), 2);
    // a corner grip resizes about the opposite corner and keeps the proportions
    let e = s.doc(|d| d.by_id(id).unwrap().clone());
    let moved = s.doc(|d| grip_edit(&e, "k2", pt(130., 80.), d));
    let Kind::Image(i) = moved.kind else { panic!() };
    near(i.p.x, 30.0);
    near(i.size().0, 100.0);
    near(i.size().1, 50.0);
}

#[test]
fn wblock_carries_the_pictures() {
    let (s, key) = with_picture();
    attach(&s, &key);
    s.click(0., 0.).ty("1").ty("0");
    s.cmd("WBLOCK")
        .click(50., 25.)
        .enter()
        .click(0., 0.)
        .ty("pic");
    let (_, dxf) =
        s.h.borrow_mut()
            .take_events()
            .into_iter()
            .find_map(|e| {
                if let Event::SaveFile { name, data, .. } = e {
                    Some((name, String::from_utf8(data).unwrap()))
                } else {
                    None
                }
            })
            .expect("file");
    assert!(dxf.contains("IMAGEDEF") && dxf.contains("plan.png"));
}
