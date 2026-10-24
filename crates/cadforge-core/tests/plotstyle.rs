//! Plot style tables: the text layout AutoCAD writes, its compressed container, and how a table looks things up.
use cadforge_core::drawing::Drawing;
use cadforge_core::plotstyle::*;
use cadforge_core::style::Color;

/// A colour dependent table in AutoCAD's text layout: colour 1 prints red at 0.5 mm, colour 2 is screened to 50 %, colour 3
/// prints black with the object's own lineweight, everything else is left alone.
fn sample_text() -> String {
    let mut t = String::new();
    t.push_str("description=\"Test table\"\naci_table_available=TRUE\nscale_factor=1.000000\napply_factor=FALSE\ncustom_lineweight_display_units=0\n");
    t.push_str("lineweight_table{\n");
    for (i, w) in [
        0.0, 0.05, 0.09, 0.10, 0.13, 0.15, 0.18, 0.20, 0.25, 0.30, 0.35, 0.40, 0.50,
    ]
    .iter()
    .enumerate()
    {
        t.push_str(&format!("\t{i}={w:.6}\n"));
    }
    t.push_str("}\naci_table{\n");
    for i in 0..255 {
        t.push_str(&format!("\t{i}=\"Color_{}\"\n", i + 1));
    }
    t.push_str("}\nplot_style{\n");
    for i in 0..255 {
        let (color, screen, lw) = match i {
            0 => (-1006698496_i64, 100, 12), // 0xC3FF0000: red, lineweight index 12 = 0.50 mm
            1 => (-1, 50, 0),
            2 => (-1006632960, 100, 0), // 0xC3000000: black
            _ => (-1, 100, 0),
        };
        t.push_str(&format!("\t{i}{{\n\t\tname=\"Color_{}\"\n\t\tlocalized_name=\"Color_{}\"\n\t\tdescription=\"\"\n\t\tcolor={color}\n\t\tmode_color=-1\n\t\tcolor_policy=1\n\t\tphysical_pen_number=0\n\t\tvirtual_pen_number=0\n\t\tscreen={screen}\n\t\tlinepattern_size=0.500000\n\t\tlinetype=0\n\t\tadaptive_angle=0.0\n\t\tlineweight={lw}\n\t\tfill_style=73\n\t\tend_style=4\n\t\tjoin_style=5\n\t}}\n", i + 1, i + 1));
    }
    t.push_str("}\n");
    t
}

fn container(text: &str, header_ints: &[u8], trailer: &[u8]) -> Vec<u8> {
    let mut out = b"PIAFILEVERSION_2.0,CTBVER1,compress\r\npmzlibcodec".to_vec();
    out.extend_from_slice(header_ints);
    out.extend(miniz_oxide::deflate::compress_to_vec_zlib(
        text.as_bytes(),
        6,
    ));
    out.extend_from_slice(trailer);
    out
}

#[test]
fn the_text_layout_gives_one_style_per_colour() {
    let t = parse_text("test", &sample_text()).unwrap();
    assert_eq!(t.name, "test");
    assert_eq!(t.description, "Test table");
    assert!(t.color_based);
    assert_eq!(t.styles.len(), 255);
    assert_eq!(t.styles[0].name, "Color_1");
    // colour 1: red, 0.50 mm (table index 12), full strength
    let red = t.style_for(Color::Aci(1)).unwrap();
    assert_eq!(red.color, Some([255, 0, 0]));
    assert_eq!(red.lineweight, Some(50));
    assert_eq!(red.screen, 100);
    // colour 2: keeps the object's colour and lineweight but is screened to 50 %
    let screened = t.style_for(Color::Aci(2)).unwrap();
    assert_eq!(
        (screened.color, screened.lineweight, screened.screen),
        (None, None, 50)
    );
    // colour 3: black
    assert_eq!(t.style_for(Color::Aci(3)).unwrap().color, Some([0, 0, 0]));
    // everything else changes nothing
    let plain = t.style_for(Color::Aci(40)).unwrap();
    assert_eq!(
        (plain.color, plain.lineweight, plain.screen),
        (None, None, 100)
    );
}

#[test]
fn objects_are_matched_to_a_style_through_their_aci_colour() {
    let t = parse_text("t", &sample_text()).unwrap();
    // a true colour object uses the style of the nearest ACI colour
    assert_eq!(t.style_for(Color::Rgb(250, 5, 5)).unwrap().name, "Color_1");
    assert_eq!(t.style_for(Color::Aci(255)).unwrap().name, "Color_255");
    assert!(
        t.style_for(Color::ByLayer).is_none(),
        "colours are resolved before the lookup"
    );
    assert!(t.named("color_2").is_some());
}

#[test]
fn the_compressed_container_gives_the_same_table() {
    let plain = parse_text("t", &sample_text()).unwrap();
    // the bytes between the marker and the zlib stream are not interpreted: any header, any trailer
    for (head, tail) in [
        (&[][..], &[][..]),
        (
            &[0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
            &[1u8, 2, 3, 4][..],
        ),
        (&[0x0A][..], &[0u8; 16][..]),
    ] {
        let bytes = container(&sample_text(), head, tail);
        let t = parse_bytes("t", &bytes).unwrap_or_else(|e| panic!("{e} (head {head:?})"));
        assert_eq!(t, plain);
    }
}

#[test]
fn plain_text_files_and_base64_work_too() {
    let plain = parse_text("t", &sample_text()).unwrap();
    assert_eq!(parse_bytes("t", sample_text().as_bytes()).unwrap(), plain);
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(container(&sample_text(), &[], &[]));
    assert_eq!(parse_base64("t", &b64).unwrap(), plain);
}

#[test]
fn damaged_files_are_refused_with_a_reason() {
    assert!(parse_bytes("t", b"")
        .unwrap_err()
        .contains("no plot styles"));
    assert!(parse_bytes("t", b"not a plot style table at all")
        .unwrap_err()
        .contains("no plot styles"));
    let mut cut = container(&sample_text(), &[], &[]);
    cut.truncate(cut.len() - 40);
    assert!(parse_bytes("t", &cut)
        .unwrap_err()
        .contains("could not be read"));
    assert!(
        parse_bytes("t", b"PIAFILEVERSION_2.0,CTBVER1,compress\r\nsomething")
            .unwrap_err()
            .contains("no data section")
    );
    assert!(parse_base64("t", "!!!").unwrap_err().contains("base64"));
}

#[test]
fn a_named_table_has_no_colour_lookup() {
    let text = "description=\"named\"\nplot_style{\n\t0{\n\t\tname=\"Normal\"\n\t\tcolor=-1\n\t\tscreen=100\n\t\tlineweight=0\n\t}\n\t1{\n\t\tname=\"Thin\"\n\t\tcolor=-1006698496\n\t\tscreen=30\n\t\tlineweight=0\n\t}\n}\n";
    let t = parse_text("named", text).unwrap();
    assert!(!t.color_based);
    assert_eq!(t.styles.len(), 2);
    assert!(t.style_for(Color::Aci(1)).is_none());
    let thin = t.named("thin").unwrap();
    assert_eq!((thin.color, thin.screen), (Some([255, 0, 0]), 30));
}

#[test]
fn the_table_is_saved_with_the_drawing() {
    let mut d = Drawing::new("t");
    d.plot_styles = Some(std::rc::Rc::new(
        parse_text("mine", &sample_text()).unwrap(),
    ));
    let text = d.to_json_string();
    let back = Drawing::from_json_str(&text, None).unwrap();
    assert_eq!(back.plot_styles, d.plot_styles);
    // a drawing without one writes nothing
    assert!(!Drawing::new("x").to_json_string().contains("plot_styles"));
    // and monochrome makes everything black
    let m = PlotStyleTable::monochrome();
    assert!((1..=255).all(|i| m.style_for(Color::Aci(i)).unwrap().color == Some([0, 0, 0])));
}
