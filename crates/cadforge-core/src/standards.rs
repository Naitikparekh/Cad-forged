//! CAD standards (STANDARDS, CHECKSTANDARDS). A drawing is associated with standards files (`.dws`, or any `.dwg` /
//! `.dxf`), and keeps a copy of the definitions each one sets: layers, linetypes and dimension, multileader and table
//! styles. Checking lists every named object that no standards file has (a non-standard name) and every one whose
//! properties differ from the standard object of the same name; fixing a problem gives the object the standard
//! definition, or moves whatever uses it onto a standard object and deletes it.
use crate::defs::LinetypeDef;
use crate::drawing::Drawing;
use crate::entities::Kind;
use crate::style::{describe_color, linetype, lw_display, DimStyle, Layer, MLeaderStyle, TableStyle};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// The definitions of one standards file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Standard {
    /// The file, as it was named when it was associated.
    pub file: String,
    #[serde(default)]
    pub layers: Vec<Layer>,
    /// Linetypes the file defines or uses (names in upper case; `CONTINUOUS` has no pattern).
    #[serde(default)]
    pub ltypes: Vec<LinetypeDef>,
    #[serde(default)]
    pub dim_styles: IndexMap<String, DimStyle>,
    #[serde(default)]
    pub mleader_styles: IndexMap<String, MLeaderStyle>,
    #[serde(default)]
    pub table_styles: IndexMap<String, TableStyle>,
}

/// The standards files of a drawing and the problems the user chose to ignore.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Standards {
    #[serde(default)]
    pub files: Vec<Standard>,
    /// [`Problem::key`]s of ignored problems.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored: Vec<String>,
}

impl Standards {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.ignored.is_empty()
    }
}

/// The kinds of named objects that are checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    Layer,
    Linetype,
    DimStyle,
    MLeaderStyle,
    TableStyle,
}

impl Table {
    pub fn label(self) -> &'static str {
        match self {
            Table::Layer => "Layer",
            Table::Linetype => "Linetype",
            Table::DimStyle => "Dimension style",
            Table::MLeaderStyle => "Multileader style",
            Table::TableStyle => "Table style",
        }
    }
}

/// One object that does not follow the standards.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub table: Table,
    pub name: String,
    /// The standards file that has an object of this name (the properties differ); `None` for a non-standard name.
    pub file: Option<String>,
    /// The properties that differ, as "Color: red (standard: 1)".
    pub diffs: Vec<String>,
    /// Standard objects that can replace this one.
    pub candidates: Vec<String>,
    /// The fix offered by default: the standard object of the same name, or one with the same properties.
    pub preferred: Option<String>,
}

impl Problem {
    /// Identifies the problem in [`Standards::ignored`].
    pub fn key(&self) -> String {
        format!("{}|{}", self.table.label(), self.name.to_uppercase())
    }
    pub fn message(&self) -> String {
        match &self.file {
            Some(f) => format!("{} \"{}\": properties do not match {f}: {}", self.table.label(), self.name, self.diffs.join("; ")),
            None => format!("{} \"{}\" is not in the standards files", self.table.label(), self.name),
        }
    }
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Layers every drawing has, which are never reported.
fn always_standard_layer(name: &str) -> bool {
    same(name, "0") || same(name, "DEFPOINTS") || name.contains('|')
}

fn plain_ltype(name: &str) -> bool {
    ["CONTINUOUS", "BYLAYER", "BYBLOCK"].iter().any(|n| same(n, name))
}

fn ltype_def(d: &Drawing, name: &str) -> LinetypeDef {
    let key = name.trim().to_uppercase();
    if let Some(c) = d.custom_ltypes.iter().find(|c| c.name == key) {
        return c.clone();
    }
    match linetype(&key) {
        Some(l) if !plain_ltype(&key) => LinetypeDef { name: key, desc: l.desc.to_string(), pattern: l.pattern.to_vec() },
        _ => LinetypeDef { name: key, desc: String::new(), pattern: vec![] },
    }
}

/// The linetypes a drawing uses or defines (upper case, without ByLayer / ByBlock).
fn ltypes_of(d: &Drawing) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut add = |n: &str| {
        let n = n.trim().to_uppercase();
        if !n.is_empty() && !same(&n, "BYLAYER") && !same(&n, "BYBLOCK") && !out.contains(&n) {
            out.push(n);
        }
    };
    for l in &d.layers {
        add(&l.ltype);
    }
    for e in d.all_entities() {
        if let Some(t) = &e.ltype {
            add(t);
        }
    }
    for c in &d.custom_ltypes {
        add(&c.name);
    }
    out
}

impl Standard {
    /// What a drawing used as a standards file sets.
    pub fn from_drawing(file: &str, d: &Drawing) -> Standard {
        let mut ltypes: Vec<LinetypeDef> = ltypes_of(d).iter().map(|n| ltype_def(d, n)).collect();
        if !ltypes.iter().any(|l| l.name == "CONTINUOUS") {
            ltypes.insert(0, LinetypeDef { name: "CONTINUOUS".into(), desc: String::new(), pattern: vec![] });
        }
        Standard {
            file: file.to_string(),
            layers: d.layers.clone(),
            ltypes,
            dim_styles: d.settings.dim_styles.clone(),
            mleader_styles: d.settings.mleader_styles.clone(),
            table_styles: d.settings.table_styles.clone(),
        }
    }
}

fn find<'a, T>(files: &'a [Standard], name: &str, list: impl Fn(&'a Standard) -> Vec<(&'a str, &'a T)>) -> Option<(&'a str, &'a T)> {
    files.iter().find_map(|s| list(s).into_iter().find(|(n, _)| same(n, name)).map(|(_, t)| (s.file.as_str(), t)))
}

fn names<'a>(files: &'a [Standard], list: impl Fn(&'a Standard) -> Vec<&'a str>) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for n in files.iter().flat_map(list) {
        if !out.iter().any(|o| same(o, n)) {
            out.push(n.to_string());
        }
    }
    out
}

fn layer_diffs(l: &Layer, s: &Layer) -> Vec<String> {
    let mut v = vec![];
    if l.color != s.color {
        v.push(format!("Color: {} (standard: {})", describe_color(Some(l.color)), describe_color(Some(s.color))));
    }
    if !same(&l.ltype, &s.ltype) {
        v.push(format!("Linetype: {} (standard: {})", l.ltype, s.ltype));
    }
    if l.lw != s.lw {
        v.push(format!("Lineweight: {} (standard: {})", lw_display(Some(l.lw)), lw_display(Some(s.lw))));
    }
    if l.plot != s.plot {
        v.push(format!("Plot: {} (standard: {})", yes_no(l.plot), yes_no(s.plot)));
    }
    if l.transp.unwrap_or(0.0) != s.transp.unwrap_or(0.0) {
        v.push(format!("Transparency: {} (standard: {})", l.transp.unwrap_or(0.0), s.transp.unwrap_or(0.0)));
    }
    v
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "Yes"
    } else {
        "No"
    }
}

/// The settings of two styles that differ, by their names in the file format ("txt: 2.5 (standard: 3.5)").
fn style_diffs<T: Serialize>(a: &T, b: &T) -> Vec<String> {
    let (Ok(serde_json::Value::Object(a)), Ok(serde_json::Value::Object(b))) = (serde_json::to_value(a), serde_json::to_value(b)) else { return vec![] };
    let show = |v: Option<&serde_json::Value>| v.map_or("-".to_string(), |v| v.to_string().trim_matches('"').to_string());
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).filter(|k| k.as_str() != "name").collect();
    keys.dedup();
    let mut seen = std::collections::BTreeSet::new();
    keys.into_iter().filter(|k| seen.insert(k.as_str()) && a.get(*k) != b.get(*k)).map(|k| format!("{k}: {} (standard: {})", show(a.get(k)), show(b.get(k)))).collect()
}

fn styles<'a, T: Serialize + PartialEq>(
    out: &mut Vec<Problem>,
    table: Table,
    mine: &'a IndexMap<String, T>,
    files: &'a [Standard],
    list: impl Fn(&'a Standard) -> &'a IndexMap<String, T> + Copy,
) {
    let all = names(files, |s| list(s).keys().map(String::as_str).collect());
    for (name, st) in mine {
        match find(files, name, |s| list(s).iter().map(|(k, v)| (k.as_str(), v)).collect()) {
            Some((file, std)) if std != st => {
                let diffs = style_diffs(st, std);
                if !diffs.is_empty() {
                    out.push(Problem { table, name: name.clone(), file: Some(file.into()), diffs, candidates: all.clone(), preferred: Some(name.clone()) });
                }
            }
            Some(_) => {}
            // every drawing has a Standard style; only a standards file that defines one says what it must be
            None if same(name, "Standard") => {}
            None => {
                let preferred = files.iter().flat_map(|s| list(s).iter()).find(|(_, v)| style_diffs(*v, st).is_empty()).map(|(k, _)| k.clone());
                out.push(Problem { table, name: name.clone(), file: None, diffs: vec![], candidates: all.clone(), preferred });
            }
        }
    }
}

/// Every problem of the drawing, in the order layers, linetypes, dimension, multileader and table styles; ignored
/// problems are left out. A drawing without standards files has none.
pub fn check(d: &Drawing) -> Vec<Problem> {
    let files = &d.standards.files;
    let mut out = vec![];
    if files.is_empty() {
        return out;
    }
    // layers
    let all = names(files, |s| s.layers.iter().map(|l| l.name.as_str()).collect());
    for l in d.layers.iter().filter(|l| !always_standard_layer(&l.name)) {
        match find(files, &l.name, |s| s.layers.iter().map(|x| (x.name.as_str(), x)).collect()) {
            Some((file, s)) => {
                let diffs = layer_diffs(l, s);
                if !diffs.is_empty() {
                    out.push(Problem { table: Table::Layer, name: l.name.clone(), file: Some(file.into()), diffs, candidates: all.clone(), preferred: Some(s.name.clone()) });
                }
            }
            None => {
                let preferred = files.iter().flat_map(|s| s.layers.iter()).find(|s| !always_standard_layer(&s.name) && layer_diffs(l, s).is_empty()).map(|s| s.name.clone());
                out.push(Problem { table: Table::Layer, name: l.name.clone(), file: None, diffs: vec![], candidates: all.clone(), preferred });
            }
        }
    }
    // linetypes
    let all = names(files, |s| s.ltypes.iter().map(|l| l.name.as_str()).collect());
    for name in ltypes_of(d).into_iter().filter(|n| !same(n, "CONTINUOUS")) {
        let mine = ltype_def(d, &name);
        match find(files, &name, |s| s.ltypes.iter().map(|x| (x.name.as_str(), x)).collect()) {
            Some((file, s)) if s.pattern != mine.pattern => {
                let diffs = vec![format!("Pattern: {:?} (standard: {:?})", mine.pattern, s.pattern)];
                out.push(Problem { table: Table::Linetype, name: name.clone(), file: Some(file.into()), diffs, candidates: all.clone(), preferred: Some(s.name.clone()) });
            }
            Some(_) => {}
            None => {
                let preferred = files.iter().flat_map(|s| s.ltypes.iter()).find(|s| !s.pattern.is_empty() && s.pattern == mine.pattern).map(|s| s.name.clone());
                out.push(Problem { table: Table::Linetype, name, file: None, diffs: vec![], candidates: all.clone(), preferred });
            }
        }
    }
    styles(&mut out, Table::DimStyle, &d.settings.dim_styles, files, |s| &s.dim_styles);
    styles(&mut out, Table::MLeaderStyle, &d.settings.mleader_styles, files, |s| &s.mleader_styles);
    styles(&mut out, Table::TableStyle, &d.settings.table_styles, files, |s| &s.table_styles);
    out.retain(|p| !d.standards.ignored.contains(&p.key()));
    out
}

/// Give the drawing the standards file's definition of linetype `name` (built-in linetypes are the same everywhere).
fn bring_ltype(d: &mut Drawing, name: &str) {
    if plain_ltype(name) {
        return;
    }
    let def = d.standards.files.iter().flat_map(|s| s.ltypes.iter()).find(|l| same(&l.name, name) && !l.pattern.is_empty()).cloned();
    if let Some(def) = def {
        // (a built-in name is refused, and needs nothing)
        let _ = d.define_linetype(def);
    }
}

/// Fix one problem: `with` is the standard object to use (by default the preferred one). An object of a standard
/// name takes the standard definition; an object of a non-standard name is replaced by `with` wherever it is used
/// and deleted. Returns what was done. The caller wraps this in an undo step.
pub fn fix(d: &mut Drawing, p: &Problem, with: Option<&str>) -> Result<String, String> {
    let target = with.map(str::to_string).or_else(|| p.preferred.clone()).ok_or_else(|| format!("{} \"{}\" has no standard replacement; choose one.", p.table.label(), p.name))?;
    let Some(target) = p.candidates.iter().find(|c| same(c, &target)).cloned() else {
        return Err(format!("\"{target}\" is not a standard {}.", p.table.label().to_lowercase()));
    };
    let files = d.standards.files.clone();
    let renamed = !same(&target, &p.name);
    match p.table {
        Table::Layer => {
            let (_, s) = find(&files, &target, |s| s.layers.iter().map(|x| (x.name.as_str(), x)).collect()).ok_or("missing standard layer")?;
            bring_ltype(d, &s.ltype);
            match d.layers.iter_mut().find(|l| same(&l.name, &target)) {
                Some(l) => {
                    (l.color, l.ltype, l.lw, l.plot, l.transp) = (s.color, s.ltype.clone(), s.lw, s.plot, s.transp);
                    if renamed || l.name != s.name {
                        l.name = s.name.clone();
                    }
                }
                None => d.layers.push(Layer { on: true, frozen: false, locked: false, ..s.clone() }),
            }
            if renamed {
                let from = p.name.clone();
                for e in d.all_entities_mut().filter(|e| same(&e.layer, &from)) {
                    e.layer = s.name.clone();
                }
                for so in d.solids.iter_mut().filter(|so| same(&so.layer, &from)) {
                    so.layer = s.name.clone();
                }
                if same(&d.cur.layer, &from) {
                    d.cur.layer = s.name.clone();
                }
                d.layers.retain(|l| !same(&l.name, &from));
            }
        }
        Table::Linetype => {
            bring_ltype(d, &target);
            if renamed {
                let from = p.name.clone();
                let to = if same(&target, "CONTINUOUS") { "Continuous".to_string() } else { target.to_uppercase() };
                for l in d.layers.iter_mut().filter(|l| same(&l.ltype, &from)) {
                    l.ltype = to.clone();
                }
                for e in d.all_entities_mut() {
                    if e.ltype.as_deref().is_some_and(|t| same(t, &from)) {
                        e.ltype = Some(to.clone());
                    }
                }
                if d.cur.ltype.as_deref().is_some_and(|t| same(t, &from)) {
                    d.cur.ltype = Some(to);
                }
                d.custom_ltypes.retain(|c| !same(&c.name, &from));
            }
        }
        Table::DimStyle => {
            let (_, s) = find(&files, &target, |s| s.dim_styles.iter().map(|(k, v)| (k.as_str(), v)).collect()).ok_or("missing standard style")?;
            let key = d.settings.dim_styles.keys().find(|k| same(k, &target)).cloned().unwrap_or_else(|| target.clone());
            d.settings.dim_styles.insert(key.clone(), DimStyle { name: key.clone(), ..s.clone() });
            if renamed {
                for e in d.all_entities_mut() {
                    if let Kind::Dim(dd) = &mut e.kind {
                        if dd.ds.as_deref().is_some_and(|n| same(n, &p.name)) {
                            dd.ds = Some(key.clone());
                        }
                    }
                }
                if same(&d.settings.cur_dim_style, &p.name) {
                    d.settings.cur_dim_style = key.clone();
                }
                d.settings.dim_styles.retain(|k, _| !same(k, &p.name));
            }
        }
        Table::MLeaderStyle => {
            let (_, s) = find(&files, &target, |s| s.mleader_styles.iter().map(|(k, v)| (k.as_str(), v)).collect()).ok_or("missing standard style")?;
            let key = d.settings.mleader_styles.keys().find(|k| same(k, &target)).cloned().unwrap_or_else(|| target.clone());
            d.settings.mleader_styles.insert(key.clone(), MLeaderStyle { name: key.clone(), ..s.clone() });
            if renamed {
                if same(&d.settings.cur_mleader_style, &p.name) {
                    d.settings.cur_mleader_style = key.clone();
                }
                d.settings.mleader_styles.retain(|k, _| !same(k, &p.name));
            }
        }
        Table::TableStyle => {
            let (_, s) = find(&files, &target, |s| s.table_styles.iter().map(|(k, v)| (k.as_str(), v)).collect()).ok_or("missing standard style")?;
            let key = d.settings.table_styles.keys().find(|k| same(k, &target)).cloned().unwrap_or_else(|| target.clone());
            d.settings.table_styles.insert(key.clone(), TableStyle { name: key.clone(), ..s.clone() });
            if renamed {
                for e in d.all_entities_mut() {
                    if let Kind::Table(t) = &mut e.kind {
                        if same(&t.style, &p.name) {
                            t.style = key.clone();
                        }
                    }
                }
                if same(&d.settings.cur_table_style, &p.name) {
                    d.settings.cur_table_style = key.clone();
                }
                d.settings.table_styles.retain(|k, _| !same(k, &p.name));
            }
        }
    }
    Ok(if renamed { format!("{} \"{}\" replaced by \"{target}\".", p.table.label(), p.name) } else { format!("{} \"{}\" now has the standard properties.", p.table.label(), p.name) })
}

/// Fix every problem that has a preferred fix; returns the messages of the fixes and the problems left.
pub fn fix_all(d: &mut Drawing) -> (Vec<String>, Vec<Problem>) {
    let mut done = vec![];
    let mut tried = std::collections::HashSet::new();
    // check again after each fix: replacing a layer can change the linetype problems
    while let Some(p) = check(d).into_iter().find(|p| p.preferred.is_some() && !tried.contains(&p.key())) {
        tried.insert(p.key());
        if let Ok(m) = fix(d, &p, None) {
            done.push(m);
        }
    }
    (done, check(d))
}
