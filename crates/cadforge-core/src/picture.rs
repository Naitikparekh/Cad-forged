//! Raster images (IMAGEATTACH): the pictures a drawing carries, how they are decoded and how IMAGEADJUST changes them.
//!
//! A drawing keeps the encoded file (PNG or JPEG) so that saving the project preserves the picture; entities refer to it by
//! name ([`crate::entities::ImageData`]). The pixels are decoded lazily and reduced to at most [`MAX_SIDE`] pixels on the
//! longer side, which is plenty for any screen or plot.
use base64::Engine as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cell::OnceCell;
use std::rc::Rc;

/// Longest side of the decoded copy used for drawing.
pub const MAX_SIDE: u32 = 4096;

/// Brightness / contrast / fade that leave a picture unchanged.
pub const NEUTRAL: (i32, i32, i32) = (50, 50, 0);

/// An encoded picture file; serialised as base64.
#[derive(Clone, Debug, PartialEq)]
pub struct Bytes(pub Rc<Vec<u8>>);

impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(self.0.as_slice()))
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Bytes, D::Error> {
        let s = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD.decode(s.trim()).map(|v| Bytes(Rc::new(v))).map_err(serde::de::Error::custom)
    }
}

/// Decoded pixels: straight RGBA, row 0 at the top.
#[derive(Debug, PartialEq)]
pub struct Decoded {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Decoded {
    /// The pixels after IMAGEADJUST: `brightness` and `contrast` run 0..100 (50 = unchanged), `fade` 0..100 mixes the
    /// picture toward `bg`. Returns the pixels unchanged for the neutral setting.
    pub fn adjusted(&self, brightness: i32, contrast: i32, fade: i32, bg: [u8; 3]) -> Vec<u8> {
        if (brightness, contrast, fade) == NEUTRAL {
            return self.rgba.clone();
        }
        let offset = (brightness.clamp(0, 100) - 50) as f32 * 2.55;
        let c = contrast.clamp(0, 100) as f32;
        let k = if c <= 50.0 { c / 50.0 } else { 1.0 + (c - 50.0) / 10.0 };
        let f = fade.clamp(0, 100) as f32 / 100.0;
        let mut out = self.rgba.clone();
        for px in out.chunks_exact_mut(4) {
            for (i, v) in px[..3].iter_mut().enumerate() {
                let x = ((*v as f32 - 128.0) * k + 128.0 + offset).clamp(0.0, 255.0);
                *v = (x * (1.0 - f) + bg[i] as f32 * f).round().clamp(0.0, 255.0) as u8;
            }
        }
        out
    }
}

/// Encode RGBA pixels as a PNG.
pub fn encode_png(w: u32, h: u32, rgba: Vec<u8>) -> Option<Vec<u8>> {
    let img = image::RgbaImage::from_raw(w, h, rgba)?;
    let mut out = vec![];
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).ok()?;
    Some(out)
}

/// A picture a drawing carries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageDef {
    /// File name the picture was attached from (also what a DXF file refers to).
    pub path: String,
    /// Pixel size of the original picture.
    pub w: u32,
    pub h: u32,
    /// The encoded file; `None` when the drawing only names a file that is not available (an imported DXF).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Bytes>,
    #[serde(skip)]
    decoded: OnceCell<Option<Rc<Decoded>>>,
}

impl PartialEq for ImageDef {
    fn eq(&self, o: &ImageDef) -> bool {
        self.path == o.path && self.w == o.w && self.h == o.h && self.data == o.data
    }
}

impl ImageDef {
    /// Read a PNG / JPEG file.
    pub fn from_file(path: &str, bytes: Vec<u8>) -> Result<ImageDef, String> {
        let fmt = image::guess_format(&bytes).map_err(|_| format!("{path}: not a PNG or JPEG image."))?;
        if !matches!(fmt, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
            return Err(format!("{path}: only PNG and JPEG images are supported."));
        }
        let (w, h) = image::ImageReader::with_format(std::io::Cursor::new(&bytes), fmt)
            .into_dimensions()
            .map_err(|e| format!("{path}: {e}"))?;
        if w == 0 || h == 0 {
            return Err(format!("{path}: the image is empty."));
        }
        Ok(ImageDef { path: path.to_string(), w, h, data: Some(Bytes(Rc::new(bytes))), decoded: OnceCell::new() })
    }

    /// A reference to a picture that cannot be loaded (its size comes from the DXF file, 0 when unknown).
    pub fn missing(path: &str, w: u32, h: u32) -> ImageDef {
        ImageDef { path: path.to_string(), w, h, data: None, decoded: OnceCell::new() }
    }

    pub fn is_missing(&self) -> bool {
        self.data.is_none()
    }

    /// The decoded pixels, or `None` when the picture is missing or damaged.
    pub fn decoded(&self) -> Option<Rc<Decoded>> {
        self.decoded
            .get_or_init(|| {
                let bytes = &self.data.as_ref()?.0;
                let img = image::load_from_memory(bytes).ok()?;
                let img = if img.width().max(img.height()) > MAX_SIDE {
                    let k = MAX_SIDE as f64 / img.width().max(img.height()) as f64;
                    img.resize(((img.width() as f64 * k).round() as u32).max(1), ((img.height() as f64 * k).round() as u32).max(1), image::imageops::FilterType::Triangle)
                } else {
                    img
                };
                let rgba = img.to_rgba8();
                Some(Rc::new(Decoded { w: rgba.width(), h: rgba.height(), rgba: rgba.into_raw() }))
            })
            .clone()
    }

    /// A PNG of the picture as adjusted (for SVG and PDF output).
    pub fn adjusted_png(&self, brightness: i32, contrast: i32, fade: i32, bg: [u8; 3]) -> Option<Vec<u8>> {
        let d = self.decoded()?;
        encode_png(d.w, d.h, d.adjusted(brightness, contrast, fade, bg))
    }
}

/// Sutherland-Hodgman clip of `subject` against the convex polygon `clip` (either winding).
pub fn clip_to_convex(subject: &[crate::geom::Pt], clip: &[crate::geom::Pt]) -> Vec<crate::geom::Pt> {
    use crate::geom::pt;
    let area: f64 = (0..clip.len()).map(|i| clip[i].x * clip[(i + 1) % clip.len()].y - clip[(i + 1) % clip.len()].x * clip[i].y).sum();
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    let mut out: Vec<crate::geom::Pt> = subject.to_vec();
    for i in 0..clip.len() {
        if out.is_empty() {
            break;
        }
        let (a, b) = (clip[i], clip[(i + 1) % clip.len()]);
        let side = |p: crate::geom::Pt| sign * ((b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x));
        let mut next = vec![];
        for j in 0..out.len() {
            let (p, q) = (out[j], out[(j + 1) % out.len()]);
            let (sp, sq) = (side(p), side(q));
            if sp >= 0.0 {
                next.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                let t = sp / (sp - sq);
                next.push(pt(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t));
            }
        }
        out = next;
    }
    out
}

/// A key for a new picture: the file name without directory and extension, made unique among `taken`.
pub fn unique_key(path: &str, taken: impl Fn(&str) -> bool) -> String {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    let base: String = stem.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ') { c } else { '_' }).collect();
    let base = if base.trim().is_empty() { "Image".to_string() } else { base };
    if !taken(&base) {
        return base;
    }
    (2..).map(|i| format!("{base}{i}")).find(|k| !taken(k)).expect("an unused key exists")
}
