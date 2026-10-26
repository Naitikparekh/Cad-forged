//! System variables (port of `src/engine/sysvars.js`). Application preferences live in [`Sysvars::prefs`];
//! drawing variables are backed by the active [`Drawing`].
use cadforge_core::drawing::Drawing;
use cadforge_core::style::{ltype_key, Color, LINEWEIGHTS, LW_BYLAYER};
use indexmap::IndexMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Str(String),
}

impl Value {
    pub fn num(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Str(s) => s.trim().parse().ok(),
        }
    }
    pub fn text(&self) -> String {
        match self {
            Value::Num(n) => fmt_num(*n),
            Value::Str(s) => s.clone(),
        }
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Value {
        Value::Num(n)
    }
}
impl From<i64> for Value {
    fn from(n: i64) -> Value {
        Value::Num(n as f64)
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Str(s.into())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Str(s)
    }
}

/// Number to text the way JavaScript prints it (no trailing `.0`).
pub fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarType {
    Int,
    Real,
    Str,
}

fn n(v: f64) -> Value {
    Value::Num(v)
}

pub fn pref_defaults() -> IndexMap<String, Value> {
    let nums: &[(&str, f64)] = &[
        ("OSMODE", 4133.0),
        ("AUTOSNAP", 63.0),
        ("APERTURE", 10.0),
        ("PICKBOX", 3.0),
        ("GRIPSIZE", 5.0),
        ("GRIPS", 2.0),
        ("GRIPOBJLIMIT", 100.0),
        ("PICKFIRST", 1.0),
        ("PICKADD", 2.0),
        ("PICKAUTO", 1.0),
        ("PICKDRAG", 0.0),
        ("CURSORSIZE", 5.0),
        ("DYNMODE", 3.0),
        ("POLARANG", 90.0),
        ("ORTHOMODE", 0.0),
        ("SNAPMODE", 0.0),
        ("GRIDMODE", 1.0),
        ("GRIDMAJOR", 5.0),
        ("GRIDUNIT_X", 10.0),
        ("GRIDUNIT_Y", 10.0),
        ("SNAPUNIT_X", 10.0),
        ("SNAPUNIT_Y", 10.0),
        ("SNAPANG", 0.0),
        ("SNAPSTYL", 0.0),
        ("ISOPLANE", 0.0),
        ("ISAVEBAK", 1.0),
        ("LWDISPLAY", 0.0),
        ("TRANSPARENCYDISPLAY", 1.0),
        ("FILLMODE", 1.0),
        ("HIGHLIGHT", 1.0),
        ("SELECTIONPREVIEW", 3.0),
        ("SELECTIONCYCLING", 0.0),
        ("SPLMETHOD", 0.0),
        ("SPLDEGREE", 3.0),
        ("VPCONTROL", 1.0),
        ("MENUBAR", 0.0),
        ("LAYLOCKFADECTL", 50.0),
        ("MAXHATCH", 60000.0),
        ("TOOLTIPS", 1.0),
        ("NAVVCUBE", 1.0),
        ("NAVBAR", 1.0),
        ("TOUCHMODE", 1.0),
        ("UCSICON", 1.0),
        ("UCSORG", 0.0),
        ("QPMODE", -1.0),
        ("OTRACKMODE", 1.0),
        ("POLARMODE", 0.0),
        ("SHORTCUTMENU", 11.0),
        ("INPUTHISTORYMODE", 15.0),
        ("AUTOCOMPLETE", 1.0),
        ("TRACEWID", 0.05),
        ("INPUTSEARCHDELAY", 300.0),
        ("SAVETIME", 10.0),
        ("QTEXTMODE", 0.0),
        ("LAYERPMODE", 1.0),
    ];
    let mut m: IndexMap<String, Value> = nums.iter().map(|(k, v)| (k.to_string(), n(*v))).collect();
    m.insert("THEME".into(), Value::Str("dark".into()));
    m.insert("BACKGROUND".into(), Value::Str(String::new()));
    m.insert("PLINEWID".into(), n(0.0));
    m.insert("POLARADDANG".into(), Value::Str(String::new()));
    m.insert("LOCALE".into(), Value::Str("ENU".into()));
    m
}

/// The interface languages LOCALE takes: English, German, French, Spanish.
pub const LOCALES: [&str; 4] = ["ENU", "DEU", "FRA", "ESP"];

const DRAWING_VARS: &[(&str, VarType)] = &[
    ("LTSCALE", VarType::Real),
    ("CELTSCALE", VarType::Real),
    ("PDMODE", VarType::Int),
    ("PDSIZE", VarType::Real),
    ("DIMASSOC", VarType::Int),
    ("CANNOSCALE", VarType::Str),
    ("TEXTSIZE", VarType::Real),
    ("FILLETRAD", VarType::Real),
    ("CHAMFERA", VarType::Real),
    ("CHAMFERB", VarType::Real),
    ("OFFSETDIST", VarType::Real),
    ("LUPREC", VarType::Int),
    ("AUPREC", VarType::Int),
    ("INSUNITS", VarType::Int),
    ("CLAYER", VarType::Str),
    ("CECOLOR", VarType::Str),
    ("CELTYPE", VarType::Str),
    ("CELWEIGHT", VarType::Int),
    ("DIMSCALE", VarType::Real),
    ("DIMTXT", VarType::Real),
    ("DIMASZ", VarType::Real),
    ("DIMEXO", VarType::Real),
    ("DIMEXE", VarType::Real),
    ("DIMGAP", VarType::Real),
    ("DIMDEC", VarType::Int),
    ("DIMCEN", VarType::Real),
    ("DIMTAD", VarType::Int),
];

const READ_ONLY: &[&str] = &["DWGNAME", "CMDNAMES", "USERR1", "LASTPOINT", "LASTANGLE", "PERIMETER", "AREA", "VPOINTX", "VPOINTY", "VPOINTZ", "CTAB"];

fn bad_value<T>(v: impl std::fmt::Display) -> Result<T, String> {
    Err(format!("Requires a valid value; {v} is not allowed."))
}

fn pos(v: f64) -> Result<f64, String> {
    if v > 0.0 {
        Ok(v)
    } else {
        bad_value(fmt_num(v))
    }
}

fn clamp_int(v: f64, lo: f64, hi: f64) -> Result<f64, String> {
    if v.fract() != 0.0 || v < lo || v > hi {
        bad_value(fmt_num(v))
    } else {
        Ok(v)
    }
}

pub fn color_text(c: Option<Color>) -> String {
    match c {
        None | Some(Color::ByLayer) => "BYLAYER".into(),
        Some(Color::ByBlock) => "BYBLOCK".into(),
        Some(Color::Aci(i)) => i.to_string(),
        Some(c @ Color::Rgb(..)) => c.rgb_hex().expect("rgb"),
    }
}

/// Parse a colour name/number/`r,g,b`/`#rrggbb`; `None` means BYLAYER.
pub fn parse_color(v: &str) -> Result<Option<Color>, String> {
    let t = v.trim().to_uppercase();
    match t.as_str() {
        "BYLAYER" => return Ok(None),
        "BYBLOCK" => return Ok(Some(Color::ByBlock)),
        "RED" => return Ok(Some(Color::Aci(1))),
        "YELLOW" => return Ok(Some(Color::Aci(2))),
        "GREEN" => return Ok(Some(Color::Aci(3))),
        "CYAN" => return Ok(Some(Color::Aci(4))),
        "BLUE" => return Ok(Some(Color::Aci(5))),
        "MAGENTA" => return Ok(Some(Color::Aci(6))),
        "WHITE" => return Ok(Some(Color::Aci(7))),
        _ => {}
    }
    if t.len() == 7 && t.starts_with('#') {
        if let Some(c) = Color::from_hex(&t.to_lowercase()) {
            return Ok(Some(c));
        }
    }
    if let Ok(i) = t.parse::<u32>() {
        if (1..=255).contains(&i) {
            return Ok(Some(Color::Aci(i as u8)));
        }
    }
    let parts: Vec<&str> = t.split(',').collect();
    if parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|c| c.is_ascii_digit())) {
        let v: Vec<u32> = parts.iter().map(|p| p.parse().unwrap_or(999)).collect();
        if v.iter().all(|&x| x <= 255) {
            return Ok(Some(Color::Rgb(v[0] as u8, v[1] as u8, v[2] as u8)));
        }
    }
    bad_value(v)
}

fn cur_dim(d: &mut Drawing) -> &mut cadforge_core::style::DimStyle {
    let name = d.settings.cur_dim_style.clone();
    if !d.settings.dim_styles.contains_key(&name) {
        d.settings.dim_styles.insert(name.clone(), Default::default());
    }
    d.settings.dim_styles.get_mut(&name).expect("dim style exists")
}

fn drawing_get(name: &str, d: &Drawing) -> Option<Value> {
    let st = &d.settings;
    let dim = st.cur_dim();
    Some(match name {
        "LTSCALE" => n(st.ltscale),
        "CELTSCALE" => n(st.celtscale),
        "PDMODE" => n(st.pdmode as f64),
        "DIMASSOC" => n(st.dimassoc as f64),
        "CANNOSCALE" => Value::Str(st.cannoscale.clone()),
        "PDSIZE" => n(st.pdsize),
        "TEXTSIZE" => n(st.text_height),
        "FILLETRAD" => n(st.fillet_radius),
        "CHAMFERA" => n(st.chamfer_d1),
        "CHAMFERB" => n(st.chamfer_d2),
        "OFFSETDIST" => n(st.offset_gap),
        "LUPREC" => n(st.units.precision as f64),
        "AUPREC" => n(st.units.ang_precision as f64),
        "INSUNITS" => n(st.units.ins_units as f64),
        "CLAYER" => Value::Str(d.cur.layer.clone()),
        "CECOLOR" => Value::Str(color_text(d.cur.color)),
        "CELTYPE" => Value::Str(d.cur.ltype.clone().unwrap_or_else(|| "BYLAYER".into())),
        "CELWEIGHT" => n(d.cur.lw.unwrap_or(LW_BYLAYER) as f64),
        "DIMSCALE" => n(dim.scale),
        "DIMTXT" => n(dim.txt),
        "DIMASZ" => n(dim.asz),
        "DIMEXO" => n(dim.exo),
        "DIMEXE" => n(dim.exe),
        "DIMGAP" => n(dim.gap),
        "DIMDEC" => n(dim.dec as f64),
        "DIMCEN" => n(dim.cen),
        "DIMTAD" => n(if dim.tad { 1.0 } else { 0.0 }),
        _ => return None,
    })
}

fn drawing_set(name: &str, d: &mut Drawing, v: &Value) -> Result<(), String> {
    let text = v.text();
    let num = || v.num().ok_or_else(|| format!("Requires a valid value; {text} is not allowed."));
    match name {
        "LTSCALE" => d.settings.ltscale = pos(num()?)?,
        "CELTSCALE" => d.settings.celtscale = pos(num()?)?,
        "PDMODE" => d.settings.pdmode = num()? as i32,
        "CANNOSCALE" => {
            d.set_annotation_scale(&v.text(), false)?;
        }
        "DIMASSOC" => {
            let v = num()?;
            if !(0.0..=2.0).contains(&v) {
                return Err("DIMASSOC must be 0, 1 or 2.".into());
            }
            d.settings.dimassoc = v as i32;
        }
        "PDSIZE" => d.settings.pdsize = num()?,
        "TEXTSIZE" => d.settings.text_height = pos(num()?)?,
        "FILLETRAD" => d.settings.fillet_radius = num()?.max(0.0),
        "CHAMFERA" => d.settings.chamfer_d1 = num()?.max(0.0),
        "CHAMFERB" => d.settings.chamfer_d2 = num()?.max(0.0),
        "OFFSETDIST" => d.settings.offset_gap = num()?,
        "LUPREC" => d.settings.units.precision = clamp_int(num()?, 0.0, 8.0)? as i32,
        "AUPREC" => d.settings.units.ang_precision = clamp_int(num()?, 0.0, 8.0)? as i32,
        "INSUNITS" => d.settings.units.ins_units = clamp_int(num()?, 0.0, 24.0)? as i32,
        "CLAYER" => d.set_current_layer(&text)?,
        "CECOLOR" => d.cur.color = parse_color(&text)?,
        "CELTYPE" => {
            d.cur.ltype = if text.eq_ignore_ascii_case("bylayer") { None } else { Some(ltype_key(&text).map(str::to_string).ok_or_else(|| format!("Requires a valid value; {text} is not allowed."))?) }
        }
        "CELWEIGHT" => {
            let w = num()? as i32;
            d.cur.lw = if w == LW_BYLAYER {
                None
            } else if LINEWEIGHTS.contains(&w) || w == -2 || w == -3 {
                Some(w)
            } else {
                return bad_value(w);
            };
        }
        "DIMSCALE" => cur_dim(d).scale = pos(num()?)?,
        "DIMTXT" => cur_dim(d).txt = pos(num()?)?,
        "DIMASZ" => cur_dim(d).asz = num()?.max(0.0),
        "DIMEXO" => cur_dim(d).exo = num()?.max(0.0),
        "DIMEXE" => cur_dim(d).exe = num()?.max(0.0),
        "DIMGAP" => cur_dim(d).gap = num()?.max(0.0),
        "DIMDEC" => cur_dim(d).dec = clamp_int(num()?, 0.0, 8.0)? as i32,
        "DIMCEN" => cur_dim(d).cen = num()?.max(0.0),
        "DIMTAD" => cur_dim(d).tad = num()? != 0.0,
        _ => return Err(format!("Unknown variable name {name}.")),
    }
    Ok(())
}

fn read_only_get(name: &str, d: Option<&Drawing>) -> Option<Value> {
    Some(match name {
        "DWGNAME" => Value::Str(d.map(|d| d.name.clone()).unwrap_or_default()),
        "CMDNAMES" | "LASTPOINT" => Value::Str(String::new()),
        "USERR1" | "LASTANGLE" | "PERIMETER" | "AREA" | "VPOINTX" | "VPOINTY" => n(0.0),
        "VPOINTZ" => n(1.0),
        "CTAB" => Value::Str(match d {
            Some(d) if d.space != "model" => d.space.clone(),
            _ => "Model".into(),
        }),
        _ => return None,
    })
}

#[derive(Clone, Debug)]
pub struct Sysvars {
    pub prefs: IndexMap<String, Value>,
    /// Names changed since the UI last looked (drained by the app).
    pub changed: Vec<String>,
}

impl Default for Sysvars {
    fn default() -> Sysvars {
        Sysvars { prefs: pref_defaults(), changed: vec![] }
    }
}

impl Sysvars {
    pub fn has(&self, name: &str) -> bool {
        let u = name.to_uppercase();
        self.prefs.contains_key(&u) || DRAWING_VARS.iter().any(|(k, _)| *k == u) || READ_ONLY.contains(&u.as_str())
    }

    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.prefs.keys().cloned().chain(DRAWING_VARS.iter().map(|(k, _)| k.to_string())).chain(READ_ONLY.iter().map(|k| k.to_string())).filter(|k| !k.contains('_')).collect();
        v.sort();
        v.dedup();
        v
    }

    pub fn var_type(&self, name: &str) -> VarType {
        let u = name.to_uppercase();
        if let Some((_, t)) = DRAWING_VARS.iter().find(|(k, _)| *k == u) {
            return *t;
        }
        if READ_ONLY.contains(&u.as_str()) {
            return VarType::Str;
        }
        match self.prefs.get(&u) {
            Some(Value::Str(_)) => VarType::Str,
            Some(Value::Num(v)) if v.fract() == 0.0 => VarType::Int,
            _ => VarType::Real,
        }
    }

    pub fn is_read_only(&self, name: &str) -> bool {
        READ_ONLY.contains(&name.to_uppercase().as_str())
    }

    pub fn get(&self, name: &str, doc: Option<&Drawing>) -> Result<Value, String> {
        let u = name.to_uppercase();
        if DRAWING_VARS.iter().any(|(k, _)| *k == u) {
            return doc.and_then(|d| drawing_get(&u, d)).ok_or_else(|| "No drawing is open.".to_string());
        }
        if let Some(v) = read_only_get(&u, doc) {
            return Ok(v);
        }
        self.prefs.get(&u).cloned().ok_or_else(|| format!("Unknown variable name {u}."))
    }

    pub fn set(&mut self, name: &str, value: impl Into<Value>, doc: Option<&mut Drawing>) -> Result<(), String> {
        let u = name.to_uppercase();
        let value = value.into();
        if READ_ONLY.contains(&u.as_str()) {
            return Err(format!("{u} is read-only."));
        }
        if let Some((_, ty)) = DRAWING_VARS.iter().find(|(k, _)| *k == u) {
            let v = if *ty == VarType::Str {
                Value::Str(value.text())
            } else {
                let x = value.num().filter(|x| x.is_finite());
                match x {
                    Some(x) if *ty == VarType::Int && x.fract() != 0.0 => return bad_value(value.text()),
                    Some(x) => Value::Num(x),
                    None => return bad_value(value.text()),
                }
            };
            let doc = doc.ok_or("No drawing is open.")?;
            doc.will_change();
            let r = drawing_set(&u, doc, &v);
            doc.touch();
            r?;
        } else if u == "LOCALE" {
            // the code, or its first two letters (de, fr, es, en)
            let t = value.text().trim().to_uppercase();
            let code = LOCALES.iter().find(|c| **c == t || (t.len() == 2 && c.starts_with(&t))).ok_or_else(|| format!("LOCALE must be one of {}.", LOCALES.join(", ")))?;
            self.prefs.insert(u.clone(), Value::Str(code.to_string()));
        } else if let Some(cur) = self.prefs.get(&u).cloned() {
            let new = match cur {
                Value::Str(_) => Value::Str(value.text()),
                Value::Num(c) => {
                    let v = match value.num().filter(|x| x.is_finite()) {
                        Some(v) => v,
                        None => return bad_value(value.text()),
                    };
                    Value::Num(if c.fract() == 0.0 && v.fract() != 0.0 && u != "LAYLOCKFADECTL" { v.round() } else { v })
                }
            };
            self.prefs.insert(u.clone(), new);
        } else {
            return Err(format!("Unknown variable name {u}."));
        }
        self.changed.push(u);
        Ok(())
    }

    // ---- typed access to preferences (hot path)

    pub fn int(&self, name: &str) -> i64 {
        match self.prefs.get(name) {
            Some(Value::Num(v)) => *v as i64,
            _ => 0,
        }
    }
    pub fn real(&self, name: &str) -> f64 {
        match self.prefs.get(name) {
            Some(Value::Num(v)) => *v,
            _ => 0.0,
        }
    }
    pub fn string(&self, name: &str) -> String {
        match self.prefs.get(name) {
            Some(Value::Str(s)) => s.clone(),
            Some(Value::Num(v)) => fmt_num(*v),
            None => String::new(),
        }
    }

    // ---- convenience flags used throughout the engine

    pub fn ortho(&self) -> bool {
        self.int("ORTHOMODE") != 0
    }
    pub fn polar(&self) -> bool {
        self.int("POLARMODE") != 0
    }
    pub fn otrack(&self) -> bool {
        self.int("OTRACKMODE") != 0
    }
    pub fn osnap_on(&self) -> bool {
        self.int("OSMODE") & 16384 == 0
    }
    pub fn grid_on(&self) -> bool {
        self.int("GRIDMODE") != 0
    }
    pub fn snap_on(&self) -> bool {
        self.int("SNAPMODE") != 0
    }
    /// Isometric snap and grid (SNAPSTYL 1).
    pub fn isometric(&self) -> bool {
        self.int("SNAPSTYL") == 1
    }
    pub fn dyn_mode(&self) -> i64 {
        self.int("DYNMODE")
    }
    pub fn pointer_input(&self) -> bool {
        self.int("DYNMODE") & 1 == 1
    }
    pub fn dimension_input(&self) -> bool {
        self.int("DYNMODE") & 2 == 2
    }
}

/// The additional polar tracking angles of POLARADDANG, in degrees ("5;35.5", separated by semicolons or commas).
pub fn polar_add_angles(s: &str) -> Vec<f64> {
    s.split([';', ',']).filter_map(|a| a.trim().parse::<f64>().ok()).filter(|a| a.is_finite()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_takes_the_interface_languages() {
        let mut sv = Sysvars::default();
        assert_eq!(sv.string("LOCALE"), "ENU");
        sv.set("LOCALE", "deu", None).unwrap();
        assert_eq!(sv.string("LOCALE"), "DEU");
        sv.set("LOCALE", "fr", None).unwrap();
        assert_eq!(sv.string("LOCALE"), "FRA");
        assert_eq!(sv.set("LOCALE", "JPN", None), Err("LOCALE must be one of ENU, DEU, FRA, ESP.".to_string()));
        assert_eq!(sv.string("LOCALE"), "FRA", "a bad value changes nothing");
    }

    #[test]
    fn prefs_and_drawing_vars() {
        let mut sv = Sysvars::default();
        let mut d = Drawing::new("t");
        assert_eq!(sv.get("osmode", Some(&d)).unwrap(), Value::Num(4133.0));
        sv.set("OSMODE", 1.0, Some(&mut d)).unwrap();
        assert_eq!(sv.int("OSMODE"), 1);
        sv.set("APERTURE", 7.6, Some(&mut d)).unwrap();
        assert_eq!(sv.int("APERTURE"), 8, "integer prefs round");
        assert!(sv.set("APERTURE", "abc", Some(&mut d)).is_err());
        sv.set("LTSCALE", 2.5, Some(&mut d)).unwrap();
        assert_eq!(d.settings.ltscale, 2.5);
        assert!(sv.set("LTSCALE", 0.0, Some(&mut d)).is_err());
        assert!(sv.set("LUPREC", 9.0, Some(&mut d)).is_err());
        assert!(sv.set("LUPREC", 2.5, Some(&mut d)).is_err());
        sv.set("CECOLOR", "red", Some(&mut d)).unwrap();
        assert_eq!(sv.get("CECOLOR", Some(&d)).unwrap().text(), "1");
        sv.set("CECOLOR", "10,20,30", Some(&mut d)).unwrap();
        assert_eq!(sv.get("CECOLOR", Some(&d)).unwrap().text(), "#0a141e");
        sv.set("CELTYPE", "dashed", Some(&mut d)).unwrap();
        assert_eq!(d.cur.ltype.as_deref(), Some("DASHED"));
        assert!(sv.set("CELTYPE", "nope", Some(&mut d)).is_err());
        sv.set("DIMSCALE", 3.0, Some(&mut d)).unwrap();
        assert_eq!(d.settings.cur_dim().scale, 3.0);
        assert!(sv.set("DWGNAME", "x", Some(&mut d)).is_err());
        assert!(sv.get("NOPE", Some(&d)).is_err());
        assert!(sv.has("clayer") && sv.has("DWGNAME") && !sv.has("NOPE"));
        assert_eq!(sv.var_type("OSMODE"), VarType::Int);
        assert_eq!(sv.var_type("THEME"), VarType::Str);
        assert_eq!(sv.var_type("CLAYER"), VarType::Str);
    }

    #[test]
    fn colour_parsing() {
        assert_eq!(parse_color("bylayer").unwrap(), None);
        assert_eq!(parse_color("ByBlock").unwrap(), Some(Color::ByBlock));
        assert_eq!(parse_color("40").unwrap(), Some(Color::Aci(40)));
        assert_eq!(parse_color("#FF8000").unwrap(), Some(Color::Rgb(255, 128, 0)));
        assert!(parse_color("300").is_err());
        assert!(parse_color("1,2,300").is_err());
        assert_eq!(color_text(Some(Color::Aci(5))), "5");
    }
}
