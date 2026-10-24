//! Colors (AutoCAD Color Index + true color), linetypes, lineweights, default styles and number formatting
//! (port of `src/core/style.js`).
use indexmap::IndexMap;
use serde::de::{self, Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use crate::defs::LinetypeDef;
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------- colors

/// An entity / layer color: ACI index, true color, or an inheritance marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Aci(u8),
    Rgb(u8, u8, u8),
    ByBlock,
    ByLayer,
}

impl Color {
    pub const WHITE: Color = Color::Aci(7);
    pub fn rgb_hex(&self) -> Option<String> {
        match self {
            Color::Rgb(r, g, b) => Some(format!("#{:02x}{:02x}{:02x}", r, g, b)),
            _ => None,
        }
    }
    pub fn from_hex(s: &str) -> Option<Color> {
        let s = s.strip_prefix('#')?;
        if s.len() != 6 {
            return None;
        }
        let n = u32::from_str_radix(s, 16).ok()?;
        Some(Color::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Color::Aci(i) => s.serialize_u64(*i as u64),
            Color::Rgb(..) => s.serialize_str(&self.rgb_hex().unwrap()),
            Color::ByBlock => s.serialize_str("BYBLOCK"),
            Color::ByLayer => s.serialize_str("BYLAYER"),
        }
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Color;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "an ACI number, \"#rrggbb\", \"BYBLOCK\" or \"BYLAYER\"")
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Color, E> {
                Ok(Color::Aci(v.min(255) as u8))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Color, E> {
                Ok(Color::Aci(v.clamp(0, 255) as u8))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Color, E> {
                Ok(Color::Aci(v.clamp(0.0, 255.0) as u8))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Color, E> {
                match v.to_ascii_uppercase().as_str() {
                    "BYBLOCK" => Ok(Color::ByBlock),
                    "BYLAYER" => Ok(Color::ByLayer),
                    _ => Color::from_hex(v).ok_or_else(|| E::custom("bad color")),
                }
            }
        }
        d.deserialize_any(V)
    }
}

const STD: [&str; 10] = ["#000000", "#ff0000", "#ffff00", "#00ff00", "#00ffff", "#0000ff", "#ff00ff", "#ffffff", "#808080", "#c0c0c0"];
const GRAYS: [&str; 6] = ["#333333", "#505050", "#696969", "#828282", "#bebebe", "#ffffff"];

fn hsv(h: f64, s: f64, v: f64) -> String {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (0.0, c, x)
    };
    let hex = |n: f64| format!("{:02x}", ((n + m) * 255.0 + 1e-6).floor() as u32);
    format!("#{}{}{}", hex(r), hex(g), hex(b))
}

fn aci_table() -> &'static Vec<String> {
    static T: OnceLock<Vec<String>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = vec![String::new(); 256];
        for i in 0..10 {
            t[i] = STD[i].to_string();
        }
        let vs = [1.0, 1.0, 0.8, 0.8, 0.6, 0.6, 0.5, 0.5, 0.3, 0.3];
        for i in 10..250 {
            let hue = (((i - 10) / 10) * 15) as f64;
            let k = (i - 10) % 10;
            t[i] = hsv(hue, if k % 2 == 1 { 0.5 } else { 1.0 }, vs[k]);
        }
        for i in 250..256 {
            t[i] = GRAYS[i - 250].to_string();
        }
        t
    })
}

/// Hex string of an ACI index.
pub fn aci_hex(i: u8) -> &'static str {
    aci_table()[i as usize].as_str()
}

/// RGB of an ACI index.
pub fn aci_rgb(i: u8) -> [u8; 3] {
    parse_hex(aci_hex(i))
}

pub fn parse_hex(s: &str) -> [u8; 3] {
    let n = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0xffffff);
    [(n >> 16) as u8, (n >> 8) as u8, n as u8]
}

pub fn aci_name(i: u8) -> Option<&'static str> {
    Some(match i {
        1 => "Red",
        2 => "Yellow",
        3 => "Green",
        4 => "Cyan",
        5 => "Blue",
        6 => "Magenta",
        7 => "White",
        _ => return None,
    })
}

/// Nearest ACI index for a true color (used when writing DXF).
pub fn rgb_to_aci(rgb: [u8; 3]) -> u8 {
    let mut best = 7u8;
    let mut bd = i64::MAX;
    for i in 1..256usize {
        let c = parse_hex(&aci_table()[i]);
        let d: i64 = (0..3).map(|k| (rgb[k] as i64 - c[k] as i64).pow(2)).sum();
        if d < bd {
            bd = d;
            best = i as u8;
        }
    }
    best
}

/// Resolve a color to RGB. ACI 7 is white on dark backgrounds and black on light ones. `contrast` darkens bright
/// ACI colors for light backgrounds.
pub fn color_rgb(c: Color, dark: bool, contrast: bool) -> [u8; 3] {
    match c {
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Aci(7) | Color::ByLayer | Color::ByBlock => {
            if dark {
                [255, 255, 255]
            } else {
                [0, 0, 0]
            }
        }
        Color::Aci(i) => {
            let rgb = aci_rgb(i);
            if contrast && !dark {
                darken_for_light(rgb)
            } else {
                rgb
            }
        }
    }
}

fn darken_for_light(c: [u8; 3]) -> [u8; 3] {
    let l = (0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64) / 255.0;
    if l <= 0.5 {
        return c;
    }
    let k = 0.5 / l;
    [(c[0] as f64 * k).round() as u8, (c[1] as f64 * k).round() as u8, (c[2] as f64 * k).round() as u8]
}

pub fn describe_color(c: Option<Color>) -> String {
    match c {
        None | Some(Color::ByLayer) => "ByLayer".into(),
        Some(Color::ByBlock) => "ByBlock".into(),
        Some(Color::Aci(i)) => aci_name(i).map(|s| s.to_string()).unwrap_or_else(|| i.to_string()),
        Some(c @ Color::Rgb(..)) => c.rgb_hex().unwrap().to_uppercase(),
    }
}

// ---------------------------------------------------------------- linetypes

pub struct LineType {
    pub name: &'static str,
    pub desc: &'static str,
    /// Positive = dash, negative = gap, 0 = dot (drawing units at LTSCALE 1).
    pub pattern: &'static [f64],
}

/// The standard linetype library (acad.lin): dash lengths in drawing units at LTSCALE 1, negative = gap, 0 = dot.
pub const LINETYPES: &[LineType] = &[
    LineType { name: "Continuous", desc: "Solid line", pattern: &[] },
    LineType { name: "BORDER", desc: "Border __ __ . __ __ . __ __ . __ __ . __ __ .", pattern: &[0.5, -0.25, 0.5, -0.25, 0.0, -0.25] },
    LineType { name: "BORDER2", desc: "Border (.5x) __.__.__.__.__.__.__.__.__.__.__.", pattern: &[0.25, -0.125, 0.25, -0.125, 0.0, -0.125] },
    LineType { name: "BORDERX2", desc: "Border (2x) ____  ____  .  ____  ____  .  ___", pattern: &[1.0, -0.5, 1.0, -0.5, 0.0, -0.5] },
    LineType { name: "CENTER", desc: "Center ____ _ ____ _ ____ _ ____ _ ____ _ ____", pattern: &[1.25, -0.25, 0.25, -0.25] },
    LineType { name: "CENTER2", desc: "Center (.5x) ___ _ ___ _ ___ _ ___ _ ___ _ ___", pattern: &[0.75, -0.125, 0.125, -0.125] },
    LineType { name: "CENTERX2", desc: "Center (2x) ________  __  ________  __  _____", pattern: &[2.5, -0.5, 0.5, -0.5] },
    LineType { name: "DASHDOT", desc: "Dash dot __ . __ . __ . __ . __ . __ . __ . __", pattern: &[0.5, -0.25, 0.0, -0.25] },
    LineType { name: "DASHDOT2", desc: "Dash dot (.5x) _._._._._._._._._._._._._._._.", pattern: &[0.25, -0.125, 0.0, -0.125] },
    LineType { name: "DASHDOTX2", desc: "Dash dot (2x) ____  .  ____  .  ____  .  ___", pattern: &[1.0, -0.5, 0.0, -0.5] },
    LineType { name: "DASHED", desc: "Dashed __ __ __ __ __ __ __ __ __ __ __ __ __ _", pattern: &[0.5, -0.25] },
    LineType { name: "DASHED2", desc: "Dashed (.5x) _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _", pattern: &[0.25, -0.125] },
    LineType { name: "DASHEDX2", desc: "Dashed (2x) ____  ____  ____  ____  ____  ___", pattern: &[1.0, -0.5] },
    LineType { name: "DIVIDE", desc: "Divide ____ . . ____ . . ____ . . ____ . . ____", pattern: &[0.5, -0.25, 0.0, -0.25, 0.0, -0.25] },
    LineType { name: "DIVIDE2", desc: "Divide (.5x) __..__..__..__..__..__..__..__.._", pattern: &[0.25, -0.125, 0.0, -0.125, 0.0, -0.125] },
    LineType { name: "DIVIDEX2", desc: "Divide (2x) ________  .  .  ________  .  .  _", pattern: &[1.0, -0.5, 0.0, -0.5, 0.0, -0.5] },
    LineType { name: "DOT", desc: "Dot . . . . . . . . . . . . . . . . . . . . . .", pattern: &[0.0, -0.25] },
    LineType { name: "DOT2", desc: "Dot (.5x) .....................................", pattern: &[0.0, -0.125] },
    LineType { name: "DOTX2", desc: "Dot (2x) .  .  .  .  .  .  .  .  .  .  .  .  .", pattern: &[0.0, -0.5] },
    LineType { name: "HIDDEN", desc: "Hidden __ __ __ __ __ __ __ __ __ __ __ __ __ __", pattern: &[0.25, -0.125] },
    LineType { name: "HIDDEN2", desc: "Hidden (.5x) _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _", pattern: &[0.125, -0.0625] },
    LineType { name: "HIDDENX2", desc: "Hidden (2x) ____ ____ ____ ____ ____ ____ ____", pattern: &[0.5, -0.25] },
    LineType { name: "PHANTOM", desc: "Phantom ______  __  __  ______  __  __  ______", pattern: &[1.25, -0.25, 0.25, -0.25, 0.25, -0.25] },
    LineType { name: "PHANTOM2", desc: "Phantom (.5x) ___ _ _ ___ _ _ ___ _ _ ___ _ _ ___", pattern: &[0.625, -0.125, 0.125, -0.125, 0.125, -0.125] },
    LineType { name: "PHANTOMX2", desc: "Phantom (2x) ____________    ____    ____    _", pattern: &[2.5, -0.5, 0.5, -0.5, 0.5, -0.5] },
    LineType { name: "ACAD_ISO02W100", desc: "ISO dash __ __ __ __ __ __ __ __ __ __ __ __ __", pattern: &[12.0, -3.0] },
    LineType { name: "ACAD_ISO03W100", desc: "ISO dash space __    __    __    __    __    __", pattern: &[12.0, -18.0] },
    LineType { name: "ACAD_ISO04W100", desc: "ISO long-dash dot ____ . ____ . ____ . ____ . _", pattern: &[24.0, -3.0, 0.0, -3.0] },
    LineType { name: "ACAD_ISO05W100", desc: "ISO long-dash double-dot ____ .. ____ .. ____ .", pattern: &[24.0, -3.0, 0.0, -3.0, 0.0, -3.0] },
    LineType { name: "ACAD_ISO06W100", desc: "ISO long-dash triple-dot ____ ... ____ ... ____", pattern: &[24.0, -3.0, 0.0, -3.0, 0.0, -3.0, 0.0, -3.0] },
    LineType { name: "ACAD_ISO07W100", desc: "ISO dot . . . . . . . . . . . . . . . . . . . .", pattern: &[0.0, -3.0] },
    LineType { name: "ACAD_ISO08W100", desc: "ISO long-dash short-dash ____ __ ____ __ ____ _", pattern: &[24.0, -3.0, 6.0, -3.0] },
    LineType { name: "ACAD_ISO09W100", desc: "ISO long-dash double-short-dash ____ __ __ ____", pattern: &[24.0, -3.0, 6.0, -3.0, 6.0, -3.0] },
    LineType { name: "ACAD_ISO10W100", desc: "ISO dash dot __ . __ . __ . __ . __ . __ . __ .", pattern: &[12.0, -3.0, 0.0, -3.0] },
    LineType { name: "ACAD_ISO11W100", desc: "ISO double-dash dot __ __ . __ __ . __ __ . __ _", pattern: &[12.0, -3.0, 12.0, -3.0, 0.0, -3.0] },
    LineType { name: "ACAD_ISO12W100", desc: "ISO dash double-dot __ . . __ . . __ . . __ . .", pattern: &[12.0, -3.0, 0.0, -3.0, 0.0, -3.0] },
    LineType { name: "ACAD_ISO13W100", desc: "ISO double-dash double-dot __ __ . . __ __ . . _", pattern: &[12.0, -3.0, 12.0, -3.0, 0.0, -3.0, 0.0, -3.0] },
    LineType { name: "ACAD_ISO14W100", desc: "ISO dash triple-dot __ . . . __ . . . __ . . . _", pattern: &[12.0, -3.0, 0.0, -3.0, 0.0, -3.0, 0.0, -3.0] },
    LineType { name: "ACAD_ISO15W100", desc: "ISO double-dash triple-dot __ __ . . . __ __ . .", pattern: &[12.0, -3.0, 12.0, -3.0, 0.0, -3.0, 0.0, -3.0, 0.0, -3.0] },
];

/// Linetypes loaded from `.lin` files (leaked once so they stay `&'static` like the built-in table).
static CUSTOM_LT: Mutex<Vec<(LinetypeDef, &'static LineType)>> = Mutex::new(Vec::new());

fn custom_lt() -> std::sync::MutexGuard<'static, Vec<(LinetypeDef, &'static LineType)>> {
    CUSTOM_LT.lock().unwrap_or_else(|e| e.into_inner())
}

/// Make a linetype from a `.lin` file available by name. The standard linetypes cannot be replaced; loading a name
/// again replaces the earlier definition.
pub fn register_linetype(def: &LinetypeDef) -> Result<&'static LineType, String> {
    let name = def.name.trim().to_uppercase();
    if name.is_empty() {
        return Err("A linetype needs a name.".into());
    }
    if def.pattern.is_empty() || def.pattern.iter().map(|v| v.abs()).sum::<f64>() <= 0.0 {
        return Err(format!("Linetype {name} has no pattern."));
    }
    if LINETYPES.iter().any(|l| l.name.eq_ignore_ascii_case(&name)) {
        return Err(format!("{name} is a standard linetype and cannot be replaced."));
    }
    let mut reg = custom_lt();
    let def = LinetypeDef { name: name.clone(), ..def.clone() };
    if let Some((_, l)) = reg.iter().find(|(d, _)| *d == def) {
        return Ok(l);
    }
    let lt: &'static LineType = Box::leak(Box::new(LineType {
        name: Box::leak(name.clone().into_boxed_str()),
        desc: Box::leak(def.desc.clone().into_boxed_str()),
        pattern: Box::leak(def.pattern.clone().into_boxed_slice()),
    }));
    reg.retain(|(d, _)| d.name != name);
    reg.push((def, lt));
    Ok(lt)
}

/// The standard linetypes followed by the ones loaded from files.
pub fn all_linetypes() -> Vec<&'static LineType> {
    LINETYPES.iter().chain(custom_lt().iter().map(|(_, l)| *l).collect::<Vec<_>>()).collect()
}

pub fn linetype(name: &str) -> Option<&'static LineType> {
    LINETYPES.iter().find(|l| l.name.eq_ignore_ascii_case(name)).or_else(|| custom_lt().iter().find(|(d, _)| d.name.eq_ignore_ascii_case(name)).map(|(_, l)| *l))
}
/// Canonical (upper-case) key of a linetype name.
pub fn ltype_key(name: &str) -> Option<&'static str> {
    linetype(name).map(|l| l.name)
}
pub fn ltype_display(name: &str) -> String {
    if name == "Continuous" || name.starts_with("ACAD_") {
        name.into()
    } else {
        let mut c = name.chars();
        c.next().map(|f| f.to_string() + &c.as_str().to_lowercase()).unwrap_or_default()
    }
}

// ---------------------------------------------------------------- lineweights (hundredths of mm)

pub const LINEWEIGHTS: [i32; 24] = [0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200, 211];
pub const LW_BYLAYER: i32 = -1;
pub const LW_BYBLOCK: i32 = -2;
pub const LW_DEFAULT_VALUE: i32 = -3;
pub const LW_DEFAULT: i32 = 25;

pub fn lw_display(v: Option<i32>) -> String {
    match v {
        None | Some(LW_BYLAYER) => "ByLayer".into(),
        Some(LW_BYBLOCK) => "ByBlock".into(),
        Some(LW_DEFAULT_VALUE) => "Default".into(),
        Some(v) => format!("{:.2} mm", v as f64 / 100.0),
    }
}

/// Screen pixels for a lineweight (hundredths of a millimetre) at 96 dpi.
pub fn lw_pixels(v: i32, lwt_on: bool) -> f64 {
    if !lwt_on {
        1.0
    } else {
        (v as f64 / 100.0 * 96.0 / 25.4 * 0.9).round().max(1.0)
    }
}

// ---------------------------------------------------------------- layers and settings

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    #[serde(default = "default_layer_color")]
    pub color: Color,
    #[serde(default = "default_ltype")]
    pub ltype: String,
    #[serde(default = "default_lw")]
    pub lw: i32,
    #[serde(default = "yes")]
    pub on: bool,
    #[serde(default)]
    pub frozen: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "yes")]
    pub plot: bool,
    #[serde(default)]
    pub desc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transp: Option<f64>,
    /// VPLAYER Vpvisdflt: frozen in viewports created from now on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub vp_frozen_default: bool,
}

fn default_layer_color() -> Color {
    Color::Aci(7)
}
fn default_ltype() -> String {
    "Continuous".into()
}
fn default_lw() -> i32 {
    LW_DEFAULT
}
fn yes() -> bool {
    true
}

impl Layer {
    pub fn new(name: &str) -> Layer {
        Layer { name: name.into(), color: Color::Aci(7), ltype: "Continuous".into(), lw: LW_DEFAULT, on: true, frozen: false, locked: false, plot: true, desc: String::new(), transp: None, vp_frozen_default: false }
    }
    pub fn with_color(mut self, c: Color) -> Layer {
        self.color = c;
        self
    }
    pub fn with_ltype(mut self, l: &str) -> Layer {
        self.ltype = l.into();
        self
    }
    pub fn with_lw(mut self, lw: i32) -> Layer {
        self.lw = lw;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DimStyle {
    pub name: String,
    pub scale: f64,
    pub txt: f64,
    pub asz: f64,
    pub exo: f64,
    pub exe: f64,
    pub gap: f64,
    pub cen: f64,
    pub dec: i32,
    #[serde(deserialize_with = "bool_or_int", serialize_with = "ser_bool_as_int")]
    pub tad: bool,
    pub tih: bool,
    pub zin: i32,
    pub lfac: f64,
    pub pre: String,
    pub suf: String,
    pub blk: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

fn bool_or_int<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    struct V;
    impl<'de> Visitor<'de> for V {
        type Value = bool;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "bool or number")
        }
        fn visit_bool<E: de::Error>(self, v: bool) -> Result<bool, E> {
            Ok(v)
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<bool, E> {
            Ok(v != 0)
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<bool, E> {
            Ok(v != 0)
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<bool, E> {
            Ok(v != 0.0)
        }
    }
    d.deserialize_any(V)
}
fn ser_bool_as_int<S: Serializer>(v: &bool, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_u8(*v as u8)
}

impl Default for DimStyle {
    fn default() -> DimStyle {
        DimStyle { name: "Standard".into(), scale: 1.0, txt: 2.5, asz: 2.5, exo: 0.625, exe: 1.25, gap: 0.625, cen: 2.5, dec: 2, tad: true, tih: false, zin: 0, lfac: 1.0, pre: String::new(), suf: String::new(), blk: "closed".into(), color: None }
    }
}

/// Multileader style (MLEADERSTYLE).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MLeaderStyle {
    pub name: String,
    pub text_height: f64,
    pub arrow: f64,
    pub landing: f64,
}

impl Default for MLeaderStyle {
    fn default() -> MLeaderStyle {
        MLeaderStyle { name: "Standard".into(), text_height: 2.5, arrow: 2.5, landing: 5.0 }
    }
}

/// Table style (TABLESTYLE).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TableStyle {
    pub name: String,
    pub text_height: f64,
    /// The first row is a merged title row with larger text.
    pub title: bool,
}

impl Default for TableStyle {
    fn default() -> TableStyle {
        TableStyle { name: "Standard".into(), text_height: 2.5, title: false }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Units {
    #[serde(rename = "type")]
    pub kind: String,
    pub precision: i32,
    pub ang_precision: i32,
    pub ins_units: i32,
}

impl Default for Units {
    fn default() -> Units {
        Units { kind: "decimal".into(), precision: 4, ang_precision: 0, ins_units: 4 }
    }
}

/// Attribute display mode (ATTDISP): `Normal` honours each attribute's own visibility.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttDisp {
    #[default]
    Normal,
    On,
    Off,
}

impl AttDisp {
    fn is_normal(&self) -> bool {
        *self == AttDisp::Normal
    }
}

/// Model-space drawing limits (LIMITS): the rectangle and whether limits checking is on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Limits {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub on: bool,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits { min_x: 0.0, min_y: 0.0, max_x: 420.0, max_y: 297.0, on: false }
    }
}

impl Limits {
    fn is_default(&self) -> bool {
        *self == Limits::default()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub units: Units,
    pub ltscale: f64,
    pub celtscale: f64,
    pub pdmode: i32,
    pub pdsize: f64,
    pub text_height: f64,
    pub text_style: String,
    pub dim_styles: IndexMap<String, DimStyle>,
    pub cur_dim_style: String,
    pub fillet_radius: f64,
    pub chamfer_d1: f64,
    pub chamfer_d2: f64,
    pub offset_gap: f64,
    pub plinewid: f64,
    #[serde(skip_serializing_if = "AttDisp::is_normal")]
    pub attdisp: AttDisp,
    #[serde(skip_serializing_if = "Limits::is_default")]
    pub limits: Limits,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_states: Option<IndexMap<String, Vec<Layer>>>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub mleader_styles: IndexMap<String, MLeaderStyle>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cur_mleader_style: String,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub table_styles: IndexMap<String, TableStyle>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cur_table_style: String,
    /// IMAGEFRAME: 0 frames hidden, 1 shown and plotted, 2 shown but not plotted.
    #[serde(skip_serializing_if = "is_one")]
    pub image_frame: u8,
    /// IMAGEQUALITY draft: pictures are drawn without smoothing.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub image_draft: bool,
}

fn is_one(v: &u8) -> bool {
    *v == 1
}

impl Default for Settings {
    fn default() -> Settings {
        let mut dim_styles = IndexMap::new();
        dim_styles.insert("Standard".to_string(), DimStyle::default());
        Settings {
            units: Units::default(),
            ltscale: 1.0,
            celtscale: 1.0,
            pdmode: 0,
            pdsize: 0.0,
            text_height: 2.5,
            text_style: "Standard".into(),
            dim_styles,
            cur_dim_style: "Standard".into(),
            fillet_radius: 0.0,
            chamfer_d1: 0.0,
            chamfer_d2: 0.0,
            offset_gap: 0.0,
            plinewid: 0.0,
            attdisp: AttDisp::Normal,
            limits: Limits::default(),
            layer_states: None,
            mleader_styles: IndexMap::new(),
            cur_mleader_style: String::new(),
            table_styles: IndexMap::new(),
            cur_table_style: String::new(),
            image_frame: 1,
            image_draft: false,
        }
    }
}

impl Settings {
    pub fn cur_dim(&self) -> &DimStyle {
        self.dim_styles.get(&self.cur_dim_style).or_else(|| self.dim_styles.values().next()).expect("a dimension style")
    }

    /// The current multileader style (the built-in Standard style until MLEADERSTYLE defines one).
    pub fn cur_mleader(&self) -> MLeaderStyle {
        self.mleader_styles.get(&self.cur_mleader_style).cloned().unwrap_or_default()
    }

    /// The current table style (the built-in Standard style until TABLESTYLE defines one).
    pub fn cur_table(&self) -> TableStyle {
        self.table_styles.get(&self.cur_table_style).cloned().unwrap_or_default()
    }
}

// ---------------------------------------------------------------- number formatting

/// Fixed-point format with `precision` decimals; `trim` removes trailing zeros.
pub fn fmt_num(x: f64, precision: usize, trim: bool) -> String {
    if !x.is_finite() {
        return x.to_string();
    }
    let x = if x.abs() < 0.5 * 10f64.powi(-(precision as i32)) { 0.0 } else { x };
    let mut s = format!("{:.*}", precision, x);
    if s.starts_with("-0") && s.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') {
        s = s.trim_start_matches('-').to_string();
    }
    if trim && s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    s
}

pub fn fmt_angle(radians: f64, precision: usize) -> String {
    format!("{}°", fmt_num(radians * 180.0 / std::f64::consts::PI, precision, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aci_palette_matches_autocad() {
        assert_eq!(aci_hex(1), "#ff0000");
        assert_eq!(aci_hex(7), "#ffffff");
        assert_eq!(aci_hex(20), "#ff3f00");
        assert_eq!(aci_hex(30), "#ff7f00");
        assert_eq!(aci_hex(250), "#333333");
        assert_eq!(rgb_to_aci([255, 0, 0]), 1);
        assert_eq!(color_rgb(Color::Aci(7), true, false), [255, 255, 255]);
        assert_eq!(color_rgb(Color::Aci(7), false, false), [0, 0, 0]);
        assert_eq!(color_rgb(Color::Aci(2), false, true)[0], 137);       // yellow is darkened on white
    }

    #[test]
    fn color_json_forms() {
        assert_eq!(serde_json::to_string(&Color::Aci(3)).unwrap(), "3");
        assert_eq!(serde_json::to_string(&Color::Rgb(255, 136, 0)).unwrap(), "\"#ff8800\"");
        assert_eq!(serde_json::from_str::<Color>("\"BYBLOCK\"").unwrap(), Color::ByBlock);
        assert_eq!(serde_json::from_str::<Color>("\"#ff8800\"").unwrap(), Color::Rgb(255, 136, 0));
        assert_eq!(serde_json::from_str::<Color>("5").unwrap(), Color::Aci(5));
    }

    #[test]
    fn settings_round_trip_with_the_web_format() {
        let json = r##"{"units":{"type":"decimal","precision":4,"angPrecision":0,"insUnits":4},"ltscale":1,"celtscale":1,"pdmode":0,"pdsize":0,"textHeight":2.5,"textStyle":"Standard","dimStyles":{"Standard":{"name":"Standard","scale":1,"txt":2.5,"asz":2.5,"exo":0.625,"exe":1.25,"gap":0.625,"cen":2.5,"dec":2,"tad":1,"tih":false,"zin":0,"lfac":1,"pre":"","suf":"","blk":"closed"}},"curDimStyle":"Standard","filletRadius":0,"chamferD1":0,"chamferD2":0,"offsetGap":0,"plinewid":0}"##;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s, Settings::default());
        // same keys and values (numbers compared as f64: the web app writes `1`, serde writes `1.0`)
        fn norm(v: serde_json::Value) -> serde_json::Value {
            match v {
                serde_json::Value::Number(n) => serde_json::json!(n.as_f64().unwrap()),
                serde_json::Value::Array(a) => serde_json::Value::Array(a.into_iter().map(norm).collect()),
                serde_json::Value::Object(o) => serde_json::Value::Object(o.into_iter().map(|(k, v)| (k, norm(v))).collect()),
                v => v,
            }
        }
        let back = norm(serde_json::to_value(&s).unwrap());
        let orig = norm(serde_json::from_str(json).unwrap());
        assert_eq!(back, orig);
    }

    #[test]
    fn limits_stay_out_of_default_files_and_round_trip() {
        let mut s = Settings::default();
        assert!(serde_json::to_value(&s).unwrap().get("limits").is_none());
        s.limits.max_x = 1000.0;
        s.limits.on = true;
        let back: Settings = serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
        assert_eq!(back.limits, s.limits);
        assert_eq!(serde_json::to_value(&s).unwrap()["limits"]["maxX"], 1000.0);
    }

    #[test]
    fn number_formatting() {
        assert_eq!(fmt_num(1.23456, 2, false), "1.23");
        assert_eq!(fmt_num(-0.00001, 2, false), "0.00");
        assert_eq!(fmt_num(2.50, 4, true), "2.5");
        assert_eq!(fmt_num(3.0, 4, true), "3");
        assert_eq!(fmt_angle(std::f64::consts::PI / 2.0, 0), "90°");
        assert_eq!(lw_display(Some(25)), "0.25 mm");
        assert_eq!(lw_display(None), "ByLayer");
        assert_eq!(ltype_display("DASHED"), "Dashed");
        assert_eq!(ltype_key("dashed"), Some("DASHED"));
        assert_eq!(describe_color(Some(Color::Aci(1))), "Red");
    }
}
