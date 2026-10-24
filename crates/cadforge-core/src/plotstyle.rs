//! Plot style tables: AutoCAD `.ctb` (colour dependent) and `.stb` (named) files.
//!
//! The text layout is the one AutoCAD writes (`description=`, `lineweight_table{ }`, `aci_table{ }`, `plot_style{ 0{ } }`).
//! A file may hold that text as is or inside AutoCAD's container: a few header lines (`PIAFILEVERSION_2.0,CTBVER1,compress`,
//! `pmzlibcodec`) followed by a zlib stream. The stream is found by looking for a zlib header after the `pmzlibcodec`
//! marker rather than by trusting a fixed layout of the bytes in between.
//!
//! What a style can change when plotting: the colour (or keep the object's), the lineweight (or keep the object's) and the
//! screening (how much of the ink reaches the paper). Linetype, end / join style, fill style and pen numbers are read
//! and kept but not applied.
use crate::style::{aci_rgb, rgb_to_aci, Color};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlotStyle {
    pub name: String,
    /// Plot colour; `None` keeps the object's colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    /// Lineweight in hundredths of a millimetre; `None` keeps the object's lineweight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineweight: Option<i32>,
    /// Screening, 0..=100 (100 = full ink).
    pub screen: u8,
    /// Physical pen number (read from the file, unused).
    #[serde(default)]
    pub pen: i32,
}

impl PlotStyle {
    fn plain(name: &str) -> PlotStyle {
        PlotStyle {
            name: name.to_string(),
            color: None,
            lineweight: None,
            screen: 100,
            pen: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlotStyleTable {
    /// File name without extension.
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// `true` for a colour dependent table (CTB): style `i` belongs to ACI colour `i + 1`. `false` for a named table.
    pub color_based: bool,
    pub styles: Vec<PlotStyle>,
}

impl PlotStyleTable {
    /// The style that applies to an object whose resolved colour is `c` (colour dependent tables only).
    pub fn style_for(&self, c: Color) -> Option<&PlotStyle> {
        if !self.color_based {
            return None;
        }
        let aci = match c {
            Color::Aci(n) if n >= 1 => n,
            Color::Aci(_) => 7,
            Color::Rgb(r, g, b) => rgb_to_aci([r, g, b]),
            Color::ByLayer | Color::ByBlock => return None,
        };
        self.styles.get(aci as usize - 1)
    }

    /// The named style called `name` (named tables).
    pub fn named(&self, name: &str) -> Option<&PlotStyle> {
        self.styles
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// A table that makes everything plot in black at full strength, like AutoCAD's monochrome.ctb.
    pub fn monochrome() -> PlotStyleTable {
        let styles = (1..=255)
            .map(|i| PlotStyle {
                name: format!("Color_{i}"),
                color: Some([0, 0, 0]),
                lineweight: None,
                screen: 100,
                pen: 0,
            })
            .collect();
        PlotStyleTable {
            name: "monochrome".into(),
            description: "Everything in black".into(),
            color_based: true,
            styles,
        }
    }
}

/// Parse a table from base64 text (how the engine receives a file's bytes through a command argument).
pub fn parse_base64(name: &str, b64: &str) -> Result<PlotStyleTable, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|_| "The plot style file data is not valid base64.".to_string())?;
    parse_bytes(name, &bytes)
}

/// Parse a plot style table from the bytes of a `.ctb` / `.stb` file.
pub fn parse_bytes(name: &str, bytes: &[u8]) -> Result<PlotStyleTable, String> {
    let text = table_text(bytes)?;
    parse_text(name, &text)
}

/// The table text of a file: either the bytes themselves or the inflated zlib stream behind `pmzlibcodec`.
fn table_text(bytes: &[u8]) -> Result<String, String> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(80)]);
    if !head.starts_with("PIAFILEVERSION") {
        return Ok(decode(bytes));
    }
    let marker = find(bytes, b"pmzlibcodec")
        .ok_or("The file claims to be a plot style table but has no data section.")?;
    // try every position behind the marker that looks like a zlib header
    for i in marker + 11..bytes.len().saturating_sub(2) {
        let (a, b) = (bytes[i], bytes[i + 1]);
        if a == 0x78 && matches!(b, 0x01 | 0x5E | 0x9C | 0xDA) {
            if let Ok(inflated) = miniz_oxide::inflate::decompress_to_vec_zlib(&bytes[i..]) {
                let text = decode(&inflated);
                if text.contains("plot_style{") || text.contains("description=") {
                    return Ok(text);
                }
            }
            // a stream with trailing bytes after it (the file's checksum) is rejected by the strict call: take what is there
            if let Ok(inflated) =
                miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&bytes[i..], 16 << 20)
            {
                let text = decode(&inflated);
                if text.contains("plot_style{") {
                    return Ok(text);
                }
            }
        }
    }
    Err("The compressed data of the plot style table could not be read.".into())
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn decode(b: &[u8]) -> String {
    let end = b.iter().rposition(|c| *c != 0).map_or(0, |i| i + 1);
    String::from_utf8_lossy(&b[..end]).into_owned()
}

fn unquote(v: &str) -> String {
    v.trim().trim_matches('"').to_string()
}

fn color_of(v: &str) -> Option<[u8; 3]> {
    let n = v.trim().parse::<i64>().ok()? as u32;
    if n == 0xFFFF_FFFF {
        return None;
    }
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

/// Parse the text form of a plot style table.
pub fn parse_text(name: &str, text: &str) -> Result<PlotStyleTable, String> {
    let mut description = String::new();
    let mut aci_available = false;
    let mut lw_table: Vec<f64> = vec![];
    let mut styles: Vec<(usize, PlotStyle, i64)> = vec![]; // (index, style, lineweight index)
    let mut stack: Vec<String> = vec![];
    let mut cur: Option<(usize, PlotStyle, i64)> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(block) = line.strip_suffix('{') {
            let block = block.trim().to_string();
            if stack.last().map(String::as_str) == Some("plot_style") {
                cur = Some((
                    block.parse().unwrap_or(styles.len()),
                    PlotStyle::plain(""),
                    0,
                ));
            }
            stack.push(block);
            continue;
        }
        if line == "}" {
            if let Some(b) = stack.pop() {
                if stack.last().map(String::as_str) == Some("plot_style")
                    && b.chars().all(|c| c.is_ascii_digit())
                {
                    if let Some(s) = cur.take() {
                        styles.push(s);
                    }
                }
            }
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        match stack.last().map(String::as_str) {
            None => match k {
                "description" => description = unquote(v),
                "aci_table_available" => aci_available = v.eq_ignore_ascii_case("true"),
                _ => {}
            },
            Some("lineweight_table") => {
                if let (Ok(i), Ok(w)) = (k.parse::<usize>(), v.parse::<f64>()) {
                    if lw_table.len() <= i {
                        lw_table.resize(i + 1, 0.0);
                    }
                    lw_table[i] = w;
                }
            }
            Some(b)
                if b.chars().all(|c| c.is_ascii_digit())
                    && stack.len() >= 2
                    && stack[stack.len() - 2] == "plot_style" =>
            {
                if let Some((_, s, lw)) = cur.as_mut() {
                    match k {
                        "name" => s.name = unquote(v),
                        "color" => s.color = color_of(v),
                        "screen" => {
                            s.screen = v
                                .parse::<f64>()
                                .map_or(100, |x| x.round().clamp(0.0, 100.0) as u8)
                        }
                        "lineweight" => *lw = v.parse().unwrap_or(0),
                        "physical_pen_number" => s.pen = v.parse().unwrap_or(0),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    if styles.is_empty() {
        return Err("The file holds no plot styles.".into());
    }
    styles.sort_by_key(|s| s.0);
    let styles: Vec<PlotStyle> = styles
        .into_iter()
        .map(|(_, mut s, lw)| {
            // index 0 of the lineweight table is "use object lineweight"
            s.lineweight = if lw > 0 {
                lw_table
                    .get(lw as usize)
                    .map(|mm| (mm * 100.0).round() as i32)
            } else {
                None
            };
            s
        })
        .collect();
    let color_based = aci_available || styles.len() >= 255;
    Ok(PlotStyleTable {
        name: name.to_string(),
        description,
        color_based,
        styles,
    })
}

/// The ACI colour a style of a colour dependent table belongs to (for listings).
pub fn aci_of_style(index: usize) -> u8 {
    (index + 1).min(255) as u8
}

/// The colour a style would print the ACI colour `aci` in, for listings (`None` keeps the object colour).
pub fn describe(table: &PlotStyleTable, aci: u8) -> String {
    match table.styles.get(aci as usize - 1) {
        None => "no style".into(),
        Some(s) => {
            let col = s.color.map_or_else(
                || format!("object colour ({:?})", aci_rgb(aci)),
                |c| format!("{},{},{}", c[0], c[1], c[2]),
            );
            let lw = s.lineweight.map_or("object lineweight".to_string(), |w| {
                format!("{:.2} mm", w as f64 / 100.0)
            });
            format!("{col}, {lw}, screening {}%", s.screen)
        }
    }
}
