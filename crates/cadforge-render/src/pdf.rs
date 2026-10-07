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

/// `ops` are in device units of `unit_mm` millimetres (y down, origin top-left); `page` is the size in device units.
pub fn write_pdf(ops: &[Op], page: (f64, f64), unit_mm: f64, title: &str) -> Vec<u8> {
    let pt = unit_mm * 72.0 / 25.4;
    let (w, h) = (page.0 * pt, page.1 * pt);
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
    let mut c: Vec<String> = vec![];
    c.push(format!("q {} 0 0 {} 0 {} cm", f(pt), f(-pt), f(h)));
    c.push("1 J 1 j".into());
    for op in ops {
        let a = match op {
            Op::Stroke { alpha, .. } | Op::Fill { alpha, .. } | Op::Text { alpha, .. } => *alpha as f64,
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
    let content = c.join("\n");
    let gs_dict = std::iter::once("/GS0 << /ca 1 /CA 1 >>".to_string()).chain(alphas.iter().map(|(a, n)| format!("/{n} << /ca {a} /CA {a} >>"))).collect::<Vec<_>>().join(" ");
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> /ExtGState << {} >> >> >>", f(w), f(h), gs_dict),
        format!("<< /Length {} >>\nstream\n{}\nendstream", content.chars().count(), content),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_string(),
        format!("<< /Title {} /Producer (CadForge) /Creator (CadForge) >>", pdf_string(title)),
    ];
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
    tail.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R /Info 6 0 R >>\nstartxref\n{}\n%%EOF\n", objs.len() + 1, xref));
    out.extend(latin1(&tail));
    out
}
