//! Minimal vector PDF writer (PDF 1.4, standard Helvetica) for recorded plot operations
//! (port of `src/io/pdf.js`).
use crate::ops::*;

fn f(v: f64) -> String {
    let s = format!("{}", (v * 1000.0).round() / 1000.0);
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn win_ansi(c: u32) -> u32 {
    match c {
        0x2300 => 0xD8,
        0x2013 => 0x96,
        0x2014 => 0x97,
        0x2018 => 0x91,
        0x2019 => 0x92,
        0x201C => 0x93,
        0x201D => 0x94,
        0x2022 => 0x95,
        0x20AC => 0x80,
        0x2122 => 0x99,
        _ => 63,
    }
}

fn pdf_string(s: &str) -> String {
    let mut out = String::from("(");
    for ch in s.chars() {
        let mut c = ch as u32;
        if c > 255 {
            c = win_ansi(c);
        }
        if c == 40 || c == 41 || c == 92 {
            out.push('\\');
            out.push(c as u8 as char);
        } else if !(32..=126).contains(&c) {
            out.push_str(&format!("\\{:03o}", c));
        } else {
            out.push(c as u8 as char);
        }
    }
    out.push(')');
    out
}

fn rgb(c: Rgb) -> String {
    format!("{} {} {}", f(c[0] as f64 / 255.0), f(c[1] as f64 / 255.0), f(c[2] as f64 / 255.0))
}

fn path_ops(c: &mut Vec<String>, paths: &[Poly]) {
    for p in paths {
        for (i, q) in p.pts.iter().enumerate() {
            c.push(format!("{} {} {}", f(q[0]), f(q[1]), if i > 0 { "l" } else { "m" }));
        }
        if p.closed {
            c.push("h".into());
        }
    }
}

struct PdfImage {
    w: u32,
    h: u32,
    rgb: Vec<u8>,
    alpha: Option<Vec<u8>>,
    smooth: bool,
}

/// One page of a PDF: recorded operations in device units and the page size in the same units.
pub struct PdfPage<'a> {
    pub ops: &'a [Op],
    pub size: (f64, f64),
}

/// The content stream of one page with the transparency states and images it uses.
fn page_stream(ops: &[Op], pt: f64, h: f64) -> (String, Vec<(f64, String)>, Vec<PdfImage>) {
    let mut alphas: Vec<(f64, String)> = vec![];
    let mut gs = |a: f64| -> String {
        let k = (a * 100.0).round() / 100.0;
        if let Some((_, n)) = alphas.iter().find(|(x, _)| *x == k) {
            return n.clone();
        }
        let name = format!("GS{}", alphas.len() + 1);
        alphas.push((k, name.clone()));
        name
    };
    let mut images: Vec<PdfImage> = vec![];
    let mut c: Vec<String> = vec![];
    c.push(format!("q {} 0 0 {} 0 {} cm", f(pt), f(-pt), f(h)));
    c.push("1 J 1 j".into());
    for op in ops {
        let a = match op {
            Op::Stroke { alpha, .. } | Op::Fill { alpha, .. } | Op::Text { alpha, .. } | Op::Image { alpha, .. } => *alpha as f64,
        };
        if a <= 0.0 {
            continue;
        }
        if a < 1.0 {
            c.push(format!("/{} gs", gs(a)));
        } else {
            c.push("/GS0 gs".into());
        }
        match op {
            Op::Stroke { paths, color, width, dash, round, .. } => {
                let cap = if *round { 1 } else { 0 };
                c.push(format!("{} RG {} w {} J {} j", rgb(*color), f(width.max(0.01)), cap, cap));
                c.push(if dash.is_empty() { "[] 0 d".into() } else { format!("[{}] 0 d", dash.iter().map(|d| f(*d)).collect::<Vec<_>>().join(" ")) });
                path_ops(&mut c, paths);
                c.push("S".into());
            }
            Op::Fill { paths, color, evenodd, .. } => {
                c.push(format!("{} rg", rgb(*color)));
                path_ops(&mut c, paths);
                c.push(if *evenodd { "f*".into() } else { "f".into() });
            }
            Op::Image { pix, adj, bg, m, clip, smooth, .. } => {
                let rgba = pix.0.adjusted(adj[0], adj[1], adj[2], *bg);
                let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
                let alpha: Vec<u8> = rgba.chunks_exact(4).map(|p| p[3]).collect();
                images.push(PdfImage {
                    w: pix.0.w,
                    h: pix.0.h,
                    rgb: miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6),
                    alpha: alpha.iter().any(|a| *a != 255).then(|| miniz_oxide::deflate::compress_to_vec_zlib(&alpha, 6)),
                    smooth: *smooth,
                });
                // the unit square of the image space (y up) onto pixel space (y down), then onto the page
                let (iw, ih) = (pix.0.w as f64, pix.0.h as f64);
                let cm = [m[0] * iw, m[1] * iw, -m[2] * ih, -m[3] * ih, m[4] + m[2] * ih, m[5] + m[3] * ih];
                c.push("q".into());
                if clip.len() >= 3 {
                    path_ops(&mut c, &[Poly { pts: clip.clone(), closed: true }]);
                    c.push("W n".into());
                }
                c.push(format!("{} cm /Im{} Do Q", cm.iter().map(|v| format!("{}", (v * 1e6).round() / 1e6)).collect::<Vec<_>>().join(" "), images.len()));
            }
            Op::Text { s, x, y, m, em, color, .. } => {
                c.push(format!(
                    "q {} cm 1 0 0 -1 0 0 cm BT /F1 {} Tf {} rg {} {} Td {} Tj ET Q",
                    m.iter().map(|v| f(*v)).collect::<Vec<_>>().join(" "),
                    f(*em),
                    rgb(*color),
                    f(*x),
                    f(-y),
                    pdf_string(s)
                ));
            }
        }
    }
    c.push("Q".into());
    (c.join("\n"), alphas, images)
}

/// `ops` are in device units of `unit_mm` millimetres (y down, origin top-left); `page` is the size in device units.
pub fn write_pdf(ops: &[Op], page: (f64, f64), unit_mm: f64, title: &str) -> Vec<u8> {
    write_pdf_pages(&[PdfPage { ops, size: page }], unit_mm, title)
}

/// A PDF with one page per entry (PUBLISH): catalog, page tree, font and info come first, then each page with its
/// content stream and image XObjects.
pub fn write_pdf_pages(pages: &[PdfPage], unit_mm: f64, title: &str) -> Vec<u8> {
    let pt = unit_mm * 72.0 / 25.4;
    let latin1_str = |b: &[u8]| -> String { b.iter().map(|v| *v as char).collect() };
    let mut objs: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        String::new(), // the page tree, filled in once the pages are numbered
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_string(),
        format!("<< /Title {} /Producer (CadForge) /Creator (CadForge) >>", pdf_string(title)),
    ];
    let mut kids: Vec<usize> = vec![];
    for page in pages {
        let (w, h) = (page.size.0 * pt, page.size.1 * pt);
        let (content, alphas, images) = page_stream(page.ops, pt, h);
        let gs_dict = std::iter::once("/GS0 << /ca 1 /CA 1 >>".to_string()).chain(alphas.iter().map(|(a, n)| format!("/{n} << /ca {a} /CA {a} >>"))).collect::<Vec<_>>().join(" ");
        let page_num = objs.len() + 1;
        // the images follow the page and its content (an image, then its soft mask when it has transparency)
        let mut image_objs: Vec<String> = vec![];
        let mut xobjects: Vec<String> = vec![];
        for (i, im) in images.iter().enumerate() {
            let num = page_num + 2 + image_objs.len();
            xobjects.push(format!("/Im{} {} 0 R", i + 1, num));
            let smask = im.alpha.as_ref().map(|_| format!(" /SMask {} 0 R", num + 1)).unwrap_or_default();
            image_objs.push(format!(
                "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode{}{} /Length {} >>\nstream\n{}\nendstream",
                im.w,
                im.h,
                if im.smooth { " /Interpolate true" } else { "" },
                smask,
                im.rgb.len(),
                latin1_str(&im.rgb)
            ));
            if let Some(a) = &im.alpha {
                image_objs.push(format!("<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n{}\nendstream", im.w, im.h, a.len(), latin1_str(a)));
            }
        }
        let xobject_dict = if xobjects.is_empty() { String::new() } else { format!(" /XObject << {} >>", xobjects.join(" ")) };
        kids.push(page_num);
        objs.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> /ExtGState << {} >>{} >> >>", f(w), f(h), page_num + 1, gs_dict, xobject_dict));
        objs.push(format!("<< /Length {} >>\nstream\n{}\nendstream", content.chars().count(), content));
        objs.extend(image_objs);
    }
    objs[1] = format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.iter().map(|k| format!("{k} 0 R")).collect::<Vec<_>>().join(" "), kids.len());
    // every character is <= 0xFF, so a latin-1 byte mapping preserves the offsets
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = vec![];
    let latin1 = |s: &str| -> Vec<u8> { s.chars().map(|c| (c as u32 & 255) as u8).collect() };
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend(latin1(&format!("{} 0 obj\n{}\nendobj\n", i + 1, o)));
    }
    let xref = out.len();
    let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1);
    for o in &offsets {
        tail.push_str(&format!("{:010} 00000 n \n", o));
    }
    tail.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R /Info 4 0 R >>\nstartxref\n{}\n%%EOF\n", objs.len() + 1, xref));
    out.extend(latin1(&tail));
    out
}
