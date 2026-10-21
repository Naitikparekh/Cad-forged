//! SVG writer for recorded plot operations (port of `src/io/svg.js`).
use crate::ops::*;

fn f(v: f64) -> String {
    format!("{}", (v * 100.0).round() / 100.0)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn col(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn d(paths: &[Poly]) -> String {
    paths.iter().map(|p| format!("M{}{}", p.pts.iter().map(|q| format!("{} {}", f(q[0]), f(q[1]))).collect::<Vec<_>>().join("L"), if p.closed { "Z" } else { "" })).collect()
}

pub fn write_svg(ops: &[Op], page: (f64, f64), unit_mm: f64, title: &str) -> String {
    let mut out = vec![
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>".to_string(),
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\">", f(page.0 * unit_mm), f(page.1 * unit_mm), f(page.0), f(page.1)),
        format!("<title>{}</title>", esc(title)),
        "<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>".to_string(),
    ];
    for op in ops {
        match op {
            Op::Stroke { paths, color, alpha, width, dash, round } => {
                if *alpha <= 0.0 {
                    continue;
                }
                let cap = if *round { "round" } else { "butt" };
                let join = if *round { "round" } else { "miter" };
                out.push(format!(
                    "<path d=\"{}\" fill=\"none\" stroke=\"{}\"{} stroke-width=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\"{}/>",
                    d(paths),
                    col(*color),
                    if *alpha < 1.0 { format!(" stroke-opacity=\"{}\"", f(*alpha as f64)) } else { String::new() },
                    f(width.max(0.01)),
                    cap,
                    join,
                    if dash.is_empty() { String::new() } else { format!(" stroke-dasharray=\"{}\"", dash.iter().map(|v| f(*v)).collect::<Vec<_>>().join(" ")) }
                ));
            }
            Op::Fill { paths, color, alpha, evenodd } => {
                if *alpha <= 0.0 {
                    continue;
                }
                out.push(format!(
                    "<path d=\"{}\" fill=\"{}\"{} fill-rule=\"{}\" stroke=\"none\"/>",
                    d(paths),
                    col(*color),
                    if *alpha < 1.0 { format!(" fill-opacity=\"{}\"", f(*alpha as f64)) } else { String::new() },
                    if *evenodd { "evenodd" } else { "nonzero" }
                ));
            }
            Op::Image { pix, adj, bg, m, clip, alpha, smooth } => {
                if *alpha <= 0.0 {
                    continue;
                }
                let Some(png) = cadforge_core::picture::encode_png(pix.0.w, pix.0.h, pix.0.adjusted(adj[0], adj[1], adj[2], *bg)) else { continue };
                use base64::Engine as _;
                let data = base64::engine::general_purpose::STANDARD.encode(png);
                let id = format!("imgclip{}", out.len());
                let (open, close) = if clip.len() >= 3 {
                    out.push(format!("<clipPath id=\"{id}\"><path d=\"{}\"/></clipPath>", d(&[Poly { pts: clip.clone(), closed: true }])));
                    (format!("<g clip-path=\"url(#{id})\">"), "</g>")
                } else {
                    (String::new(), "")
                };
                out.push(format!(
                    "{open}<image transform=\"matrix({})\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\"{}{} href=\"data:image/png;base64,{data}\"/>{close}",
                    m.iter().map(|v| format!("{}", (v * 1e6).round() / 1e6)).collect::<Vec<_>>().join(" "),
                    pix.0.w,
                    pix.0.h,
                    if *alpha < 1.0 { format!(" opacity=\"{}\"", f(*alpha as f64)) } else { String::new() },
                    if *smooth { "" } else { " image-rendering=\"pixelated\"" },
                ));
            }
            Op::Text { s, x, y, m, em, color, alpha } => {
                if *alpha <= 0.0 {
                    continue;
                }
                out.push(format!(
                    "<text transform=\"matrix({})\" x=\"{}\" y=\"{}\" font-family=\"Arial, Helvetica, sans-serif\" font-size=\"{}\" fill=\"{}\"{}>{}</text>",
                    m.iter().map(|v| format!("{}", (v * 1e4).round() / 1e4)).collect::<Vec<_>>().join(" "),
                    f(*x),
                    f(*y),
                    f(*em),
                    col(*color),
                    if *alpha < 1.0 { format!(" fill-opacity=\"{}\"", f(*alpha as f64)) } else { String::new() },
                    esc(s)
                ));
            }
        }
    }
    out.push("</svg>".into());
    out.join("\n")
}
