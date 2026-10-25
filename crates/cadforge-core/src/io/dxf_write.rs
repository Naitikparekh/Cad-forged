//! DXF writer (AutoCAD R2000 / AC1015), port of `src/io/dxf-write.js`. Output is validated against ezdxf by the tests
//! in `tests/dxf.rs`.
use crate::dim::{arrow_lines, arrow_polygon, dim_geometry, DimType};
use crate::drawing::{Drawing, Paper};
use crate::entities::*;
use crate::geom::*;
use crate::hatch::pattern;
use crate::style::*;
use crate::text::{HJust, VJust};
use std::collections::HashMap;

/// Format a number the way the JS writer does (15 significant digits, no exponent).
/// The registered application name of CadForge's extended data.
pub const CADFORGE_APP: &str = "CADFORGE";

/// JSON with every character outside ASCII escaped (\uXXXX), so that it can be split anywhere into DXF strings.
pub fn ascii_json<T: serde::Serialize>(x: &T) -> String {
    let s = serde_json::to_string(x).unwrap_or_default();
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut b = [0u16; 2];
            for u in c.encode_utf16(&mut b) {
                out.push_str(&format!("\\u{:04x}", u));
            }
        }
    }
    out
}

/// Pieces of at most 250 characters (extended data strings hold 255 bytes).
pub fn xdata_chunks(s: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut rest = s;
    while !rest.is_empty() {
        let k = rest.len().min(250);
        out.push(&rest[..k]);
        rest = &rest[k..];
    }
    out
}

/// What a drawing holds that DXF and DWG have no place for, as JSON for the CADFORGE XRECORD of the named object
/// dictionary (other programs keep it as it is): named views, the last view, the current and named UCSs, the current
/// colour / linetype / lineweight, loaded linetypes and hatch patterns, the plot style table, which blocks are external
/// references, the extrusion histories of 3D solids (their outlines named by the handles in `handles`, CadForge id to
/// handle), and the pictures' data (DXF and DWG only name picture files). None when there is nothing to keep.
pub fn cadforge_extras(doc: &Drawing, handles: &HashMap<u64, String>) -> Option<String> {
    use serde_json::{to_value, Map, Value};
    let mut m = Map::new();
    let mut put = |k: &str, v: Result<Value, serde_json::Error>| {
        if let Ok(v) = v {
            m.insert(k.into(), v);
        }
    };
    if !doc.views.is_empty() {
        put("views", to_value(&doc.views));
    }
    if let Some(v) = &doc.view {
        put("view", to_value(v));
    }
    if !doc.ucs.is_world() {
        put("ucs", to_value(doc.ucs));
    }
    if !doc.ucs_named.is_empty() {
        put("ucs_named", to_value(&doc.ucs_named));
    }
    if doc.cur.color.is_some() || doc.cur.ltype.is_some() || doc.cur.lw.is_some() {
        put("cur", to_value(&doc.cur));
    }
    if !doc.custom_ltypes.is_empty() {
        put("ltypes", to_value(&doc.custom_ltypes));
    }
    if !doc.custom_patterns.is_empty() {
        put("patterns", to_value(&doc.custom_patterns));
    }
    if let Some(p) = &doc.plot_styles {
        put("plot_styles", to_value(p.as_ref()));
    }
    // the styles as CadForge has them (the DIMSTYLE table holds what AutoCAD reads; multileader and table styles are
    // only here)
    let st = &doc.settings;
    let defaults = crate::style::Settings::default();
    if st.dim_styles != defaults.dim_styles || st.cur_dim_style != defaults.cur_dim_style {
        put("dim_styles", to_value(&st.dim_styles));
        put("cur_dim_style", to_value(&st.cur_dim_style));
    }
    if !st.mleader_styles.is_empty() || !st.cur_mleader_style.is_empty() {
        put("mleader_styles", to_value(&st.mleader_styles));
        put("cur_mleader_style", to_value(&st.cur_mleader_style));
    }
    if !st.table_styles.is_empty() || !st.cur_table_style.is_empty() {
        put("table_styles", to_value(&st.table_styles));
        put("cur_table_style", to_value(&st.cur_table_style));
    }
    if !doc.standards.is_empty() {
        put("standards", to_value(&doc.standards));
    }
    // a solid's extrusion history, with its outlines by handle (they get new ids when the file is read); left out when
    // an outline is no longer in the drawing
    let programs: Vec<Value> = doc
        .solids
        .iter()
        .filter_map(|s| {
            let p = s.program.as_ref()?;
            let mut v = to_value(p).ok()?;
            for (f, j) in p.features.iter().zip(v["features"].as_array_mut()?) {
                j["profile"] = f.profile.iter().map(|id| handles.get(id).cloned()).collect::<Option<Vec<String>>>()?.into();
            }
            Some(serde_json::json!({ "solid": s.id, "program": v }))
        })
        .collect();
    if !programs.is_empty() {
        put("programs", Ok(Value::Array(programs)));
    }
    // external references keep their contents as blocks; the link to the other drawing goes here
    let xrefs: indexmap::IndexMap<&String, &crate::drawing::XrefInfo> = doc.blocks.iter().filter_map(|(k, b)| b.xref.as_ref().map(|x| (k, x))).collect();
    if !xrefs.is_empty() {
        put("xrefs", to_value(&xrefs));
    }
    let pictures: indexmap::IndexMap<&String, &crate::picture::ImageDef> = doc.images.iter().filter(|(_, d)| d.data.is_some()).collect();
    if !pictures.is_empty() {
        put("images", to_value(&pictures));
    }
    (!m.is_empty()).then(|| ascii_json(&Value::Object(m)))
}

/// A DIMENSION's definition point (group 10) for a CadForge dimension.
pub fn dim_def_point(d: &crate::dim::DimData) -> Pt {
    let zero = pt(0.0, 0.0);
    match d.kind {
        DimType::Rad | DimType::Jog | DimType::Ord => d.c.unwrap_or(zero),
        DimType::Dia => {
            let (c, p) = (d.c.unwrap_or(zero), d.pt.unwrap_or(zero));
            pt(2.0 * c.x - p.x, 2.0 * c.y - p.y)
        }
        _ => d.dp.unwrap_or(zero),
    }
}

pub fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    if EXACT.with(|e| e.get()) {
        return format!("{}", v + 0.0);
    }
    let r: f64 = format!("{:.14e}", v).parse().unwrap_or(v);
    format!("{}", r + 0.0)
}

thread_local! {
    /// Numbers written in full (the shortest text that reads back as the same double) instead of 15 digits.
    static EXACT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// DXF text with every number exactly as stored, for encoders that read it straight back (the DWG writer), so a
/// drawing saved as DWG keeps its coordinates to the last bit.
pub fn write_dxf_exact(doc: &Drawing) -> String {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            EXACT.with(|e| e.set(false));
        }
    }
    EXACT.with(|e| e.set(true));
    let _reset = Reset;
    write_dxf(doc)
}

fn clean(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_nl = false;
    for c in s.chars() {
        if c == '\r' || c == '\n' {
            if !in_nl {
                out.push(' ');
            }
            in_nl = true;
        } else {
            in_nl = false;
            out.push(c);
        }
    }
    out
}

/// R2000 files are code-page encoded: write anything outside ASCII as `\U+XXXX` (and the usual `%%` symbols).
fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\u{2300}' => out.push_str("%%c"),
            '\u{00B0}' => out.push_str("%%d"),
            '\u{00B1}' => out.push_str("%%p"),
            c if (' '..='~').contains(&c) || c == '\r' || c == '\n' || c == '\t' => out.push(c),
            c if (c as u32) > 0xFFFF => out.push('?'),
            c => out.push_str(&format!("\\U+{:04X}", c as u32)),
        }
    }
    out
}

fn jh_code(j: HJust) -> i32 {
    match j {
        HJust::Left => 0,
        HJust::Center => 1,
        HJust::Right => 2,
    }
}
fn jv_code(j: VJust) -> i32 {
    match j {
        VJust::Baseline => 0,
        VJust::Bottom => 1,
        VJust::Middle => 2,
        VJust::Top => 3,
    }
}
fn attach(jh: HJust, jv: VJust) -> i32 {
    let col = jh_code(jh);
    let row = match jv {
        VJust::Top => 0,
        VJust::Middle => 1,
        VJust::Bottom | VJust::Baseline => 2,
    };
    row * 3 + col + 1
}

fn insunits_ok(u: i32) -> bool {
    matches!(u, 0 | 1 | 2 | 4 | 5 | 6)
}

#[derive(Default)]
struct Out {
    lines: Vec<String>,
}

impl Out {
    fn n(&mut self, code: i32, v: f64) -> &mut Self {
        self.lines.push(code.to_string());
        self.lines.push(num(v));
        self
    }
    fn i(&mut self, code: i32, v: i64) -> &mut Self {
        self.lines.push(code.to_string());
        self.lines.push(v.to_string());
        self
    }
    fn s(&mut self, code: i32, v: &str) -> &mut Self {
        self.lines.push(code.to_string());
        self.lines.push(enc(v));
        self
    }
    /// Point with group codes 10+i / 20+i / 30+i.
    fn pt(&mut self, p: Pt, i: i32) -> &mut Self {
        self.n(10 + i, p.x).n(20 + i, p.y).n(30 + i, 0.0)
    }
    fn text(&self) -> String {
        let mut s = self.lines.join("\r\n");
        s.push_str("\r\n");
        s
    }
}

/// Entity properties relevant to the common DXF group codes.
struct Props<'a> {
    layer: &'a str,
    ltype: Option<&'a str>,
    color: Option<Color>,
    lw: Option<i32>,
}

impl<'a> Props<'a> {
    fn of(e: &'a Entity) -> Props<'a> {
        Props { layer: &e.layer, ltype: e.ltype.as_deref(), color: e.color, lw: e.lw }
    }
    /// Only layer and colour (the parts a dimension passes on to its generated geometry).
    fn sub(e: &'a Entity) -> Props<'a> {
        Props { layer: &e.layer, ltype: None, color: e.color, lw: None }
    }
}

struct DimBlock {
    name: String,
    rec: String,
    blk: String,
    end: String,
    layer: String,
    lines: Vec<String>,
}

struct EOut {
    out: Out,
    owner: String,
    block_name: String,
}

struct W<'a> {
    doc: &'a Drawing,
    handle: u32,
    dim_blocks: Vec<DimBlock>,
    block_rec: HashMap<String, (String, String, String)>,
    /// Next viewport id in the layout being written (id 1 is the sheet's own viewport).
    vp_id: i64,
    /// IMAGEDEF handle of each picture used, with the (reactor, IMAGE entity) handles that point at it.
    image_defs: Vec<(String, String, Vec<(String, String)>)>,
    /// LAYER table handle of each layer (key: lower-cased name), for the frozen-layer list of viewports (group 331).
    layer_handles: HashMap<String, String>,
    /// Groups by CadForge id, with their GROUP handle and the handles of the entities written for their members.
    groups: Vec<(String, String, Vec<String>)>,
    /// The group of the entity being written (an index into `groups`); only in model space and layouts.
    cur_grp: Option<usize>,
    grouping: bool,
    /// The handle each entity was written with (its first object), by CadForge id: solids' extrusion histories name
    /// their outlines by it.
    handle_of: HashMap<u64, String>,
}

impl<'a> W<'a> {
    fn h(&mut self) -> String {
        let s = format!("{:X}", self.handle);
        self.handle += 1;
        s
    }

    fn common(&mut self, o: &mut EOut, ty: &str, p: &Props, sub: Option<&str>, layer: Option<&str>) {
        let hnd = self.h();
        o.out.s(0, ty).s(5, &hnd);
        if let Some(k) = self.cur_grp.filter(|_| ty != "ATTRIB") {
            // a group member names its group among its reactors
            o.out.s(102, "{ACAD_REACTORS").s(330, &self.groups[k].1).s(102, "}");
            self.groups[k].2.push(hnd.clone());
        }
        o.out.s(330, &o.owner.clone()).s(100, "AcDbEntity");
        if o.block_name.starts_with("*Paper_Space") {
            o.out.i(67, 1);
        }
        let layer = layer.unwrap_or(p.layer);
        o.out.s(8, if layer.is_empty() { "0" } else { layer });
        if let Some(lt) = p.ltype {
            let lt = if lt == "BYBLOCK" { "ByBlock" } else { lt };
            if lt == "ByBlock" || linetype(lt).is_some() {
                o.out.s(6, lt);
            }
        }
        match p.color {
            Some(Color::ByBlock) => {
                o.out.i(62, 0);
            }
            Some(Color::Aci(n)) => {
                o.out.i(62, n as i64);
            }
            Some(Color::Rgb(r, g, b)) => {
                o.out.i(62, rgb_to_aci([r, g, b]) as i64).i(420, ((r as i64) << 16) | ((g as i64) << 8) | b as i64);
            }
            Some(Color::ByLayer) | None => {}
        }
        if let Some(lw) = p.lw {
            if lw != LW_BYLAYER {
                o.out.i(370, lw as i64);
            }
        }
        if let Some(sub) = sub {
            o.out.s(100, sub);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, o: &mut EOut, p: Pt, h: f64, s: &str, rot: f64, jh: HJust, jv: VJust, props: &Props, layer: Option<&str>) {
        self.common(o, "TEXT", props, Some("AcDbText"), layer);
        o.out.pt(p, 0).n(40, h).s(1, &clean(s)).n(50, deg(rot)).s(7, "Standard");
        if jh_code(jh) != 0 || jv_code(jv) != 0 {
            o.out.i(72, jh_code(jh) as i64);
            o.out.pt(p, 1);
        }
        o.out.s(100, "AcDbText").i(73, jv_code(jv) as i64);
    }

    /// A layout VIEWPORT (`view_h` is the height of the model region shown, in model units).
    #[allow(clippy::too_many_arguments)]
    fn viewport(&mut self, o: &mut EOut, c: Pt, w: f64, h: f64, id: i64, status: i64, view_c: Pt, view_h: f64, locked: bool, off: bool, frozen: &[String], props: &Props) {
        self.common(o, "VIEWPORT", props, Some("AcDbViewport"), None);
        o.out.pt(c, 0).n(40, w).n(41, h).i(68, if off { 0 } else { status }).i(69, id);
        o.out.n(12, view_c.x).n(22, view_c.y).n(13, 0.0).n(23, 0.0).n(14, 10.0).n(24, 10.0).n(15, 10.0).n(25, 10.0);
        o.out.n(16, 0.0).n(26, 0.0).n(36, 1.0).n(17, 0.0).n(27, 0.0).n(37, 0.0).n(42, 50.0).n(43, 0.0).n(44, 0.0).n(45, view_h).n(50, 0.0).n(51, 0.0).i(72, 100);
        for name in frozen {
            if let Some(h) = self.layer_handles.get(&name.to_lowercase()) {
                o.out.s(331, h);
            }
        }
        o.out.i(90, if id == 1 { 557088 } else if locked { 16384 } else { 0 }).s(1, "").i(281, 0).i(71, 0).i(74, 0);
        o.out.n(110, 0.0).n(120, 0.0).n(130, 0.0).n(111, 1.0).n(121, 0.0).n(131, 0.0).n(112, 0.0).n(122, 1.0).n(132, 0.0).i(79, 0).n(146, 0.0).i(282, 0);
    }

    /// IMAGE entity; the picture itself stays outside the file (IMAGEDEF carries its path), as in AutoCAD.
    fn image(&mut self, o: &mut EOut, im: &ImageData, props: &Props) {
        let Some(def) = self.doc.images.get(&im.name) else { return };
        let (wpx, hpx) = (def.w.max(1) as f64, def.h.max(1) as f64);
        let def_h = match self.image_defs.iter().position(|d| d.0 == im.name) {
            Some(i) => self.image_defs[i].1.clone(),
            None => {
                let h = self.h();
                self.image_defs.push((im.name.clone(), h.clone(), vec![]));
                h
            }
        };
        let reactor = self.h();
        let ent_h = format!("{:X}", self.handle);
        self.common(o, "IMAGE", props, Some("AcDbRasterImage"), None);
        if let Some(d) = self.image_defs.iter_mut().find(|d| d.0 == im.name) {
            d.2.push((reactor.clone(), ent_h));
        }
        let (up, vp) = (im.u * (1.0 / wpx), im.v * (1.0 / hpx));
        let active = !im.clip.is_empty() && !im.clip_off;
        o.out.i(90, 0).pt(im.p, 0).pt(up, 1).pt(vp, 2).n(13, wpx).n(23, hpx).s(340, &def_h);
        o.out.i(70, 3 | if active { 4 } else { 0 }).i(280, active as i64).i(281, im.brightness as i64).i(282, im.contrast as i64).i(283, im.fade as i64).s(360, &reactor);
        if im.clip.is_empty() {
            o.out.i(71, 1).i(91, 2).n(14, -0.5).n(24, -0.5).n(14, wpx - 0.5).n(24, hpx - 0.5);
        } else {
            // pixel coordinates with the origin at the top-left corner of the picture (y down), pixel centres at whole numbers
            let det = im.u.x * im.v.y - im.u.y * im.v.x;
            o.out.i(71, 2).i(91, im.clip.len() as i64 + 1);
            let px = |q: Pt| {
                let d = q - im.p;
                if det.abs() < 1e-18 {
                    return (0.0, 0.0);
                }
                let (s, t) = ((d.x * im.v.y - d.y * im.v.x) / det, (im.u.x * d.y - im.u.y * d.x) / det);
                (s * wpx - 0.5, (1.0 - t) * hpx - 0.5)
            };
            for q in im.clip.iter().chain(im.clip.first()) {
                let (x, y) = px(*q);
                o.out.n(14, x).n(24, y);
            }
        }
    }

    fn line(&mut self, o: &mut EOut, a: Pt, b: Pt, props: &Props, layer: Option<&str>) {
        self.common(o, "LINE", props, Some("AcDbLine"), layer);
        o.out.pt(a, 0).pt(b, 1);
    }

    fn solid(&mut self, o: &mut EOut, pts: &[Pt], props: &Props, layer: Option<&str>) {
        self.common(o, "SOLID", props, Some("AcDbTrace"), layer);
        let p = if pts.len() == 3 { [pts[0], pts[1], pts[2], pts[2]] } else { [pts[0], pts[1], pts[2], pts[3]] };
        o.out.pt(p[0], 0).pt(p[1], 1).pt(p[3], 2).pt(p[2], 3);
    }

    fn lwpoly(&mut self, o: &mut EOut, verts: &[Vertex], closed: bool, props: &Props) {
        self.common(o, "LWPOLYLINE", props, Some("AcDbPolyline"), None);
        o.out.i(90, verts.len() as i64).i(70, closed as i64).n(43, 0.0);
        for v in verts {
            o.out.n(10, v.x).n(20, v.y);
            if v.sw != 0.0 || v.ew != 0.0 {
                o.out.n(40, v.sw).n(41, v.ew);
            }
            if v.b != 0.0 {
                o.out.n(42, v.b);
            }
        }
    }

    fn write(&mut self, o: &mut EOut, e: &Entity) {
        let outer = self.cur_grp;
        self.cur_grp = match (&e.grp, self.grouping) {
            (Some(g), true) => Some(match self.groups.iter().position(|x| x.0 == *g) {
                Some(k) => k,
                None => {
                    let h = self.h();
                    self.groups.push((g.clone(), h, vec![]));
                    self.groups.len() - 1
                }
            }),
            _ => None,
        };
        self.handle_of.entry(e.id).or_insert_with(|| format!("{:X}", self.handle));
        self.write_kind(o, e);
        self.cur_grp = outer;
    }

    fn write_kind(&mut self, o: &mut EOut, e: &Entity) {
        let props = Props::of(e);
        match &e.kind {
            Kind::Line { a, b } => self.line(o, *a, *b, &props, None),
            Kind::Circle { c, r } => {
                self.common(o, "CIRCLE", &props, Some("AcDbCircle"), None);
                o.out.pt(*c, 0).n(40, *r);
            }
            Kind::Arc { c, r, a0, a1 } => {
                self.common(o, "ARC", &props, Some("AcDbCircle"), None);
                o.out.pt(*c, 0).n(40, *r).s(100, "AcDbArc").n(50, deg(*a0)).n(51, deg(*a1));
            }
            Kind::Pline { v, closed } => self.lwpoly(o, v, *closed, &props),
            Kind::Point { p } => {
                self.common(o, "POINT", &props, Some("AcDbPoint"), None);
                o.out.pt(*p, 0);
            }
            Kind::Ellipse(el) => {
                self.common(o, "ELLIPSE", &props, Some("AcDbEllipse"), None);
                o.out.pt(el.c, 0).pt(el.m, 1).n(210, 0.0).n(220, 0.0).n(230, 1.0).n(40, el.ratio);
                let sweep = ellipse_sweep(el);
                let full = sweep >= TAU - 1e-9;
                o.out.n(41, if full { 0.0 } else { el.t0 }).n(42, if full { TAU } else { el.t0 + sweep });
            }
            Kind::Spline { fit, closed } => {
                let bz = spline_beziers(fit, *closed);
                if bz.is_empty() {
                    return;
                }
                let mut ctrl = vec![bz[0].p0];
                for b in &bz {
                    ctrl.extend([b.c1, b.c2, b.p1]);
                }
                let mut knots = vec![0.0; 4];
                for i in 1..bz.len() {
                    knots.extend([i as f64; 3]);
                }
                knots.extend([bz.len() as f64; 4]);
                self.common(o, "SPLINE", &props, Some("AcDbSpline"), None);
                o.out.n(210, 0.0).n(220, 0.0).n(230, 1.0).i(70, 8).i(71, 3).i(72, knots.len() as i64).i(73, ctrl.len() as i64).i(74, 0).n(42, 1e-7).n(43, 1e-7).n(44, 1e-10);
                for k in knots {
                    o.out.n(40, k);
                }
                for p in ctrl {
                    o.out.pt(p, 0);
                }
            }
            Kind::Text(t) if t.att.is_some() => {
                let a = t.att.as_ref().expect("att");
                self.common(o, "ATTDEF", &props, Some("AcDbText"), None);
                o.out.pt(t.p, 0).n(40, t.h).s(1, &clean(&t.s)).n(50, deg(t.rot)).s(7, "Standard");
                if jh_code(t.jh) != 0 || jv_code(t.jv) != 0 {
                    o.out.i(72, jh_code(t.jh) as i64);
                    o.out.pt(t.p, 1);
                }
                let flags = a.invisible as i64 | (a.constant as i64) << 1 | (a.verify as i64) << 2 | (a.preset as i64) << 3;
                o.out.s(100, "AcDbAttributeDefinition").s(3, &clean(&a.prompt)).s(2, &a.tag).i(70, flags).i(74, jv_code(t.jv) as i64);
            }
            Kind::Text(t) => {
                if t.multi {
                    self.common(o, "MTEXT", &props, Some("AcDbMText"), None);
                    let body: Vec<String> = t.s.split('\n').map(clean).collect();
                    o.out.pt(t.p, 0).n(40, t.h).n(41, t.w.unwrap_or(0.0)).i(71, attach(t.jh, t.jv) as i64).i(72, 1).s(1, &body.join("\\P")).s(7, "Standard").n(11, t.rot.cos()).n(21, t.rot.sin()).n(31, 0.0);
                } else {
                    self.text(o, t.p, t.h, &t.s, t.rot, t.jh, t.jv, &props, None);
                }
            }
            Kind::Xline { p, d } => {
                self.common(o, "XLINE", &props, Some("AcDbXline"), None);
                o.out.pt(*p, 0).pt(*d, 1);
            }
            Kind::Ray { p, d } => {
                self.common(o, "RAY", &props, Some("AcDbRay"), None);
                o.out.pt(*p, 0).pt(*d, 1);
            }
            Kind::Insert(ins) => {
                if !self.block_rec.contains_key(&ins.name) {
                    return;
                }
                self.common(o, "INSERT", &props, Some("AcDbBlockReference"), None);
                if !ins.attribs.is_empty() {
                    o.out.i(66, 1);
                }
                o.out.s(2, &ins.name).pt(ins.p, 0).n(41, ins.sx).n(42, ins.sy).n(43, 1.0).n(50, deg(ins.rot));
                if !ins.attribs.is_empty() {
                    for a in &ins.attribs {
                        self.common(o, "ATTRIB", &props, Some("AcDbText"), None);
                        let t = &a.t;
                        o.out.pt(t.p, 0).n(40, t.h).s(1, &clean(&t.s)).n(50, deg(t.rot)).s(7, "Standard");
                        if jh_code(t.jh) != 0 || jv_code(t.jv) != 0 {
                            o.out.i(72, jh_code(t.jh) as i64);
                            o.out.pt(t.p, 1);
                        }
                        o.out.s(100, "AcDbAttribute").s(2, &a.tag).i(70, a.hidden as i64).i(74, jv_code(t.jv) as i64);
                    }
                    let hnd = self.h();
                    let layer = if props.layer.is_empty() { "0" } else { props.layer };
                    o.out.s(0, "SEQEND").s(5, &hnd).s(330, &o.owner.clone()).s(100, "AcDbEntity").s(8, layer);
                }
            }
            Kind::Table(_) | Kind::Region(_) => self.wrapped(o, e, &props),
            Kind::Wipeout(w) if w.pts.len() >= 3 => {
                // a one-pixel "image" spanning the outline's extents; the outline is its clip boundary in pixel space
                // (origin at the centre, y down)
                let (lo, hi) = w.pts.iter().fold((w.pts[0], w.pts[0]), |(lo, hi), q| (pt(lo.x.min(q.x), lo.y.min(q.y)), pt(hi.x.max(q.x), hi.y.max(q.y))));
                let (sx, sy) = (hi.x - lo.x, hi.y - lo.y);
                if sx > 0.0 && sy > 0.0 {
                    self.common(o, "WIPEOUT", &props, Some("AcDbWipeout"), None);
                    o.out.i(90, 0).pt(lo, 0).pt(pt(sx, 0.0), 1).pt(pt(0.0, sy), 2).n(13, 1.0).n(23, 1.0).s(340, "0");
                    o.out.i(70, 7).i(280, 1).i(281, 50).i(282, 50).i(283, 0).s(360, "0").i(71, 2).i(91, w.pts.len() as i64 + 1);
                    for q in w.pts.iter().chain(w.pts.first()) {
                        o.out.n(14, (q.x - lo.x) / sx - 0.5).n(24, 0.5 - (q.y - lo.y) / sy);
                    }
                }
            }
            Kind::Wipeout(_) => {}
            Kind::Image(im) => self.image(o, im, &props),
            Kind::Leader { pts, s, h } => {
                self.common(o, "LEADER", &props, Some("AcDbLeader"), None);
                // the text is the leader's MTEXT annotation, written next (so it gets the next handle), where CadForge
                // draws it: just right of the last vertex, middle left
                let note = pts.last().filter(|_| !s.is_empty()).map(|l| (*l, format!("{:X}", self.handle)));
                o.out.s(3, "Standard").i(71, 1).i(72, 0).i(73, if note.is_some() { 0 } else { 3 }).i(74, 1).i(75, 0).n(40, *h).n(41, 0.0).i(76, pts.len() as i64);
                for p in pts {
                    o.out.pt(*p, 0);
                }
                if let Some((last, hnd)) = note {
                    o.out.s(340, &hnd);
                    self.common(o, "MTEXT", &props, Some("AcDbMText"), None);
                    let body = s.replace('\\', "\\\\").replace('{', "\\{").replace('}', "\\}");
                    let body: Vec<String> = body.split('\n').map(clean).collect();
                    o.out.pt(pt(last.x + h * 0.4, last.y), 0).n(40, *h).n(41, 0.0).i(71, 4).i(72, 1).s(1, &body.join("\\P")).s(7, "Standard").n(11, 1.0).n(21, 0.0).n(31, 0.0);
                }
            }
            Kind::Hatch(ha) => {
                let pat = pattern(&ha.pattern).or_else(|| pattern("ANSI31")).expect("ANSI31 exists");
                let solid_fill = pat.solid || ha.gradient.is_some();
                self.common(o, "HATCH", &props, Some("AcDbHatch"), None);
                o.out.pt(pt(0.0, 0.0), 0).n(210, 0.0).n(220, 0.0).n(230, 1.0).s(2, if solid_fill { "SOLID" } else { &ha.pattern }).i(70, solid_fill as i64).i(71, 0).i(91, ha.loops.len() as i64);
                for (i, lp) in ha.loops.iter().enumerate() {
                    o.out.i(92, if i == 0 { 3 } else { 2 }).i(72, 1).i(73, 1).i(93, lp.len() as i64);
                    for v in lp {
                        o.out.n(10, v.x).n(20, v.y).n(42, v.b);
                    }
                    o.out.i(97, 0);
                }
                o.out.i(75, 0).i(76, 1);
                if !solid_fill {
                    o.out.n(52, deg(ha.pa)).n(41, if ha.ps == 0.0 { 1.0 } else { ha.ps }).i(77, 0).i(78, pat.families.len() as i64);
                    for f in pat.families {
                        o.out.n(53, f.a).n(43, f.ox).n(44, f.oy).n(45, f.dx).n(46, f.dy).i(79, f.dash.map_or(0, |d| d.len()) as i64);
                        for d in f.dash.unwrap_or(&[]) {
                            o.out.n(49, *d);
                        }
                    }
                }
                o.out.i(98, 0);
                if let Some(g) = &ha.gradient {
                    let rgb = |c: [u8; 3]| ((c[0] as i64) << 16) | ((c[1] as i64) << 8) | c[2] as i64;
                    o.out.i(450, 1).i(451, 0).n(460, g.angle).n(461, 0.0).i(452, g.one_color as i64).n(462, g.tint).i(453, 2).n(463, 0.0).i(421, rgb(g.c1)).n(463, 1.0).i(421, rgb(g.c2)).s(470, &g.kind);
                }
            }
            Kind::Dim(d) => self.write_dim(o, e, d),
            Kind::Viewport(v) => {
                let id = self.vp_id;
                self.vp_id += 1;
                let (vc, scale) = v.model_view(self.doc);
                let frozen: Vec<String> = v.layers.iter().filter(|l| l.frozen).map(|l| l.layer.clone()).collect();
                self.viewport(o, v.c, v.w, v.h, id, 2, vc, v.h / scale, v.locked, v.off, &frozen, &Props::of(e));
            }
        }
    }

    fn write_dim(&mut self, o: &mut EOut, e: &Entity, d: &crate::dim::DimData) {
        let geo = dim_geometry(d, &self.doc.settings);
        let sub = Props::sub(e);
        let layer = if e.layer.is_empty() { "0".to_string() } else { e.layer.clone() };
        let emit = |w: &mut W, o: &mut EOut| {
            for s in &geo.segs {
                match *s {
                    Seg::L { a, b, .. } => w.line(o, a, b, &sub, Some(&layer)),
                    Seg::A { c, r, a0, sw, .. } => {
                        w.common(o, "ARC", &sub, Some("AcDbCircle"), Some(&layer));
                        let (s0, s1) = if sw >= 0.0 { (a0, a0 + sw) } else { (a0 + sw, a0) };
                        o.out.pt(c, 0).n(40, r).s(100, "AcDbArc").n(50, deg(s0)).n(51, deg(s1));
                    }
                }
            }
            for a in &geo.arrows {
                if let Some(poly) = arrow_polygon(a) {
                    w.solid(o, &poly, &sub, Some(&layer));
                } else {
                    for (p, q) in arrow_lines(a) {
                        w.line(o, p, q, &sub, Some(&layer));
                    }
                }
            }
            let t = &geo.text;
            w.text(o, t.p, t.h, &t.s, t.rot, t.jh, t.jv, &sub, None);
        };
        let rec = DimBlock { name: format!("*D{}", self.dim_blocks.len() + 1), rec: self.h(), blk: self.h(), end: self.h(), layer: layer.clone(), lines: vec![] };
        let mut inner = EOut { out: Out::default(), owner: rec.rec.clone(), block_name: o.block_name.clone() };
        // the dimension's block holds its geometry; the DIMENSION itself is the group member
        let grp = self.cur_grp.take();
        emit(self, &mut inner);
        self.cur_grp = grp;
        let rec = DimBlock { lines: inner.out.lines, ..rec };
        // arc-length dimensions are written as three-point angular ones (other programs show their block)
        let ty = match d.kind {
            DimType::Lin => 0,
            DimType::Ali => 1,
            DimType::Dia => 3,
            DimType::Rad | DimType::Jog => 4,
            DimType::Ang | DimType::Arc => 5,
            DimType::Ord => 6 | if d.xd { 64 } else { 0 },
        };
        let zero = pt(0.0, 0.0);
        let name = rec.name.clone();
        self.dim_blocks.push(rec);
        self.common(o, "DIMENSION", &Props::of(e), Some("AcDbDimension"), None);
        o.out.s(2, &name).pt(dim_def_point(d), 0);
        let style = d.ds.as_deref().filter(|n| self.doc.settings.dim_styles.contains_key(*n)).unwrap_or("Standard");
        o.out.pt(geo.text.p, 1).i(70, 32 | ty | if d.tp.is_some() { 128 } else { 0 }).i(71, 5).s(3, style);
        if let Some(t) = &d.txt {
            if !t.is_empty() {
                o.out.s(1, t);
            }
        }
        if let Some(r) = d.tr {
            o.out.n(53, deg(r));
        }
        match d.kind {
            DimType::Lin | DimType::Ali => {
                o.out.s(100, "AcDbAlignedDimension").pt(d.p1.unwrap_or(zero), 3).pt(d.p2.unwrap_or(zero), 4);
                if d.kind == DimType::Lin {
                    o.out.n(50, deg(d.ang.unwrap_or(0.0))).s(100, "AcDbRotatedDimension");
                }
            }
            DimType::Ord => {
                o.out.s(100, "AcDbOrdinateDimension").pt(d.pt.unwrap_or(zero), 3).pt(d.dp.unwrap_or(zero), 4);
            }
            DimType::Ang | DimType::Arc => {
                o.out.s(100, "AcDb3PointAngularDimension").pt(d.p1.unwrap_or(zero), 3).pt(d.p2.unwrap_or(zero), 4).pt(d.c.unwrap_or(zero), 5);
            }
            _ => {
                o.out.s(100, if d.kind == DimType::Dia { "AcDbDiametricDimension" } else { "AcDbRadialDimension" }).pt(d.pt.unwrap_or(zero), 5).n(40, 0.0);
            }
        }
        // CadForge's own description of the dimension, so that it opens again as the same (editable) dimension
        o.out.s(1001, CADFORGE_APP);
        for chunk in xdata_chunks(&ascii_json(d)) {
            o.out.s(1000, chunk);
        }
    }

    /// A 3D solid as polyface meshes (POLYLINE flag 64: the vertices, then the triangles). Edges between two triangles
    /// in one plane are invisible, so other programs show the solid's real edges. A mesh holds at most 32767 vertices
    /// and faces: a solid within that is one mesh with its vertices in order (it reads back exactly); a larger one is
    /// split into parts, and CADFORGE extended data names the solid each part belongs to.
    fn polyface(&mut self, o: &mut EOut, s: &crate::solid::Solid) {
        use std::collections::HashMap as Map;
        let props = Props { layer: if s.layer.is_empty() { "0" } else { &s.layer }, ltype: None, color: s.color, lw: None };
        let normal = |f: &[u32; 3]| {
            let p = |k: usize| s.vertices.get(f[k] as usize).copied().unwrap_or_default();
            p(1).sub(p(0)).cross(p(2).sub(p(0))).unit()
        };
        let mut edges: Map<(u32, u32), Vec<usize>> = Map::new();
        for (i, f) in s.faces.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (f[k], f[(k + 1) % 3]);
                edges.entry((a.min(b), a.max(b))).or_default().push(i);
            }
        }
        let hidden = |a: u32, b: u32| edges.get(&(a.min(b), a.max(b))).is_some_and(|fs| fs.len() == 2 && normal(&s.faces[fs[0]]).dot(normal(&s.faces[fs[1]])) > 1.0 - 1e-9);
        let tag = ascii_json(&serde_json::json!({ "solid": s.name, "id": s.id }));
        let whole = s.vertices.len() <= 32_767 && s.faces.len() <= 32_767;
        for part in s.faces.chunks(if whole { s.faces.len().max(1) } else { 10_000 }) {
            // the vertices this part uses (all of them for a whole solid), numbered from 1 in their order
            let mut verts: Vec<u32> = if whole { (0..s.vertices.len() as u32).collect() } else { part.iter().flatten().copied().collect() };
            verts.sort_unstable();
            verts.dedup();
            let index: Map<u32, i64> = verts.iter().enumerate().map(|(i, v)| (*v, i as i64 + 1)).collect();
            let pl = format!("{:X}", self.handle);
            self.common(o, "POLYLINE", &props, Some("AcDbPolyFaceMesh"), None);
            o.out.i(66, 1).n(10, 0.0).n(20, 0.0).n(30, 0.0).i(70, 64).i(71, verts.len() as i64).i(72, part.len() as i64);
            o.out.s(1001, CADFORGE_APP);
            for chunk in xdata_chunks(&tag) {
                o.out.s(1000, chunk);
            }
            let owner = std::mem::replace(&mut o.owner, pl.clone());
            for v in &verts {
                let p = s.vertices.get(*v as usize).copied().unwrap_or_default();
                self.common(o, "VERTEX", &props, Some("AcDbVertex"), None);
                o.out.s(100, "AcDbPolyFaceMeshVertex").n(10, p.x).n(20, p.y).n(30, p.z).i(70, 192);
            }
            for f in part {
                self.common(o, "VERTEX", &props, Some("AcDbVertex"), None);
                o.out.s(100, "AcDbFaceRecord").n(10, 0.0).n(20, 0.0).n(30, 0.0).i(70, 128);
                for k in 0..3 {
                    let (a, b) = (f[k], f[(k + 1) % 3]);
                    let i = index[&a];
                    o.out.i(71 + k as i32, if hidden(a, b) { -i } else { i });
                }
            }
            let h = self.h();
            o.out.s(0, "SEQEND").s(5, &h).s(330, &pl).s(100, "AcDbEntity").s(8, props.layer);
            o.owner = owner;
        }
    }

    /// A table or region: an insert of an anonymous block holding its lines and texts (or outlines), which is what
    /// other programs show, with CadForge's own description of it in CADFORGE extended data (left out when it would
    /// not fit in an entity's extended data; the object then opens as the block's geometry).
    fn wrapped(&mut self, o: &mut EOut, e: &Entity, props: &Props) {
        let mut k = self.dim_blocks.len() + 1;
        let name = loop {
            let n = format!("*U{k}");
            if !self.doc.blocks.contains_key(&n) && !self.dim_blocks.iter().any(|b| b.name == n) {
                break n;
            }
            k += 1;
        };
        let layer = if e.layer.is_empty() { "0".to_string() } else { e.layer.clone() };
        let rec = DimBlock { name: name.clone(), rec: self.h(), blk: self.h(), end: self.h(), layer, lines: vec![] };
        let mut inner = EOut { out: Out::default(), owner: rec.rec.clone(), block_name: o.block_name.clone() };
        let grp = self.cur_grp.take();
        match &e.kind {
            Kind::Table(t) => {
                let (segs, texts) = crate::entities::table_geometry(t);
                for sg in segs {
                    if let crate::geom::Seg::L { a, b, .. } = sg {
                        self.line(&mut inner, a, b, props, None);
                    }
                }
                for tx in texts {
                    self.text(&mut inner, tx.p, tx.h, &tx.s, tx.rot, tx.jh, tx.jv, props, None);
                }
            }
            Kind::Region(r) => {
                for l in &r.loops {
                    self.lwpoly(&mut inner, l, true, props);
                }
            }
            _ => {}
        }
        self.cur_grp = grp;
        self.dim_blocks.push(DimBlock { lines: inner.out.lines, ..rec });
        self.common(o, "INSERT", props, Some("AcDbBlockReference"), None);
        o.out.s(2, &name).pt(pt(0.0, 0.0), 0).n(41, 1.0).n(42, 1.0).n(43, 1.0).n(50, 0.0);
        let json = ascii_json(&e.kind);
        if json.len() <= 15_000 {
            o.out.s(1001, CADFORGE_APP);
            for chunk in xdata_chunks(&json) {
                o.out.s(1000, chunk);
            }
        }
    }

    fn entity_out(&mut self, list: &[&Entity], owner: &str, block_name: &str) -> Vec<String> {
        let mut o = EOut { out: Out::default(), owner: owner.to_string(), block_name: block_name.to_string() };
        for e in list {
            self.write(&mut o, e);
        }
        o.out.lines
    }
}

/// Serialise a drawing to binary DXF (the same content as [`write_dxf`], packed).
pub fn write_dxf_binary(doc: &Drawing) -> Vec<u8> {
    super::binary_dxf::text_to_binary(&write_dxf(doc))
}

/// Serialise a drawing to DXF text.
pub fn write_dxf(doc: &Drawing) -> String {
    let mut w = W { doc, handle: 0x10, dim_blocks: vec![], block_rec: HashMap::new(), vp_id: 2, image_defs: vec![], layer_handles: HashMap::new(), groups: vec![], cur_grp: None, grouping: false, handle_of: HashMap::new() };
    let mut o = Out::default();

    // ---------- collect names
    let layers: Vec<&str> = doc.layers.iter().map(|l| l.name.as_str()).collect();
    let mut used: Vec<String> = vec!["Continuous".into()];
    let add_lt = |n: &str, used: &mut Vec<String>| {
        if !used.iter().any(|u| u == n) {
            used.push(n.to_string());
        }
    };
    for l in &doc.layers {
        add_lt(if l.ltype.is_empty() { "Continuous" } else { &l.ltype }, &mut used);
    }
    for e in doc.all_entities() {
        if let Some(lt) = &e.ltype {
            if lt != "BYBLOCK" {
                add_lt(lt, &mut used);
            }
        }
    }
    let ltype_names: Vec<&'static LineType> = used.iter().filter_map(|n| linetype(n)).collect();
    let user_blocks: Vec<_> = doc.blocks.values().collect();

    // handles of fixed table objects
    let mut hs: HashMap<&str, String> = HashMap::new();
    for k in ["vport", "ltype", "layer", "style", "view", "ucs", "appid", "dimstyle", "blockRecord", "dictRoot", "dictGroup", "vportRec", "styleRec", "appidRec", "dimstyleRec", "msRec", "msBlock", "msEnd"] {
        hs.insert(k, w.h());
    }
    for b in &user_blocks {
        let t = (w.h(), w.h(), w.h());
        w.block_rec.insert(b.name.clone(), t);
    }
    // paper-space sheets: every layout owns a BLOCK_RECORD / BLOCK / ENDBLK and a LAYOUT object (an empty drawing still gets one)
    let sheets: Vec<(String, Paper, Vec<&Entity>)> = if doc.layouts.is_empty() {
        vec![("Layout1".to_string(), Paper::named("A3", true), vec![])]
    } else {
        doc.layouts.iter().map(|l| (l.name.clone(), l.paper.clone(), l.entities.iter().collect())).collect()
    };
    let sheet_block = |i: usize| if i == 0 { "*Paper_Space".to_string() } else { format!("*Paper_Space{}", i - 1) };
    struct SheetH {
        rec: String,
        blk: String,
        end: String,
        layout: String,
    }
    let sheet_h: Vec<SheetH> = sheets.iter().map(|_| SheetH { rec: w.h(), blk: w.h(), end: w.h(), layout: w.h() }).collect();
    let dict_layout = w.h();
    let model_layout = w.h();
    let layer_handle: Vec<String> = layers.iter().map(|_| w.h()).collect();
    w.layer_handles = layers.iter().zip(&layer_handle).map(|(l, h)| (l.to_lowercase(), h.clone())).collect();
    let mut ltype_handle: HashMap<String, String> = HashMap::new();
    for n in ["ByBlock", "ByLayer"].into_iter().chain(ltype_names.iter().map(|l| l.name)) {
        let hnd = w.h();
        ltype_handle.insert(n.to_string(), hnd);
    }

    // ---------- extents
    let mut bb: Option<Bounds> = None;
    for e in &doc.model.entities {
        bb = Bounds::union_opt(bb, bounds_of(e, doc));
    }
    let ext = bb.unwrap_or(Bounds { min_x: 0.0, min_y: 0.0, max_x: 1.0, max_y: 1.0 });

    // ---------- entities first (discovers dimension blocks; handles stay unique)
    let model_list: Vec<&Entity> = doc.model.entities.iter().collect();
    w.grouping = true;
    let mut model_out = w.entity_out(&model_list, &hs["msRec"], "*Model_Space");
    w.grouping = false;
    {
        let mut o = EOut { out: Out::default(), owner: hs["msRec"].clone(), block_name: "*Model_Space".into() };
        for s in &doc.solids {
            w.polyface(&mut o, s);
        }
        model_out.extend(o.out.lines);
    }
    let mut block_outs = vec![];
    for b in &user_blocks {
        let rec = w.block_rec[&b.name].0.clone();
        let list: Vec<&Entity> = b.entities.iter().collect();
        block_outs.push(w.entity_out(&list, &rec, &b.name));
    }
    let mut sheet_lines: Vec<Vec<String>> = vec![];
    w.grouping = true;
    for (i, (_, paper, list)) in sheets.iter().enumerate() {
        w.vp_id = 2;
        let bn = sheet_block(i);
        let mut o = EOut { out: Out::default(), owner: sheet_h[i].rec.clone(), block_name: bn };
        // viewport id 1: the sheet itself
        let main = Props { layer: "0", ltype: None, color: None, lw: None };
        w.viewport(&mut o, pt(paper.w / 2.0, paper.h / 2.0), paper.w, paper.h, 1, 1, pt(paper.w / 2.0, paper.h / 2.0), paper.h, false, false, &[], &main);
        for e in list {
            w.write(&mut o, e);
        }
        sheet_lines.push(o.out.lines);
    }

    // ---------- HEADER
    o.s(0, "SECTION").s(2, "HEADER");
    o.s(9, "$ACADVER").s(1, "AC1015").s(9, "$DWGCODEPAGE").s(3, "ANSI_1252");
    o.s(9, "$HANDSEED").s(5, "__HANDSEED__");
    o.s(9, "$INSBASE").pt(doc.base, 0);
    o.s(9, "$EXTMIN").pt(pt(ext.min_x, ext.min_y), 0);
    o.s(9, "$EXTMAX").pt(pt(ext.max_x, ext.max_y), 0);
    o.s(9, "$LIMMIN").n(10, 0.0).n(20, 0.0);
    o.s(9, "$LIMMAX").n(10, 420.0).n(20, 297.0);
    let st = &doc.settings;
    o.s(9, "$LTSCALE").n(40, st.ltscale);
    o.s(9, "$PDMODE").i(70, st.pdmode as i64).s(9, "$PDSIZE").n(40, st.pdsize);
    o.s(9, "$TEXTSIZE").n(40, st.text_height);
    o.s(9, "$CLAYER").s(8, &doc.cur.layer);
    let iu = st.units.ins_units;
    o.s(9, "$INSUNITS").i(70, if insunits_ok(iu) { iu } else { 0 } as i64);
    o.s(9, "$MEASUREMENT").i(70, if iu == 1 || iu == 2 { 0 } else { 1 });
    o.s(9, "$LUNITS").i(70, 2).s(9, "$LUPREC").i(70, st.units.precision as i64);
    o.s(9, "$AUPREC").i(70, st.units.ang_precision as i64);
    let ds = st.cur_dim();
    if st.dim_styles.contains_key(&st.cur_dim_style) {
        o.s(9, "$DIMSTYLE").s(2, &st.cur_dim_style);
    }
    o.s(9, "$DIMSCALE").n(40, ds.scale).s(9, "$DIMTXT").n(40, ds.txt).s(9, "$DIMASZ").n(40, ds.asz).s(9, "$DIMEXO").n(40, ds.exo).s(9, "$DIMEXE").n(40, ds.exe).s(9, "$DIMGAP").n(40, ds.gap).s(9, "$DIMDEC").i(70, ds.dec as i64).s(9, "$DIMCEN").n(40, ds.cen);
    o.s(0, "ENDSEC");

    // ---------- TABLES
    o.s(0, "SECTION").s(2, "TABLES");
    let table_start = |o: &mut Out, name: &str, hnd: &str, count: usize| {
        o.s(0, "TABLE").s(2, name).s(5, hnd).i(330, 0).s(100, "AcDbSymbolTable").i(70, count as i64);
    };
    table_start(&mut o, "VPORT", &hs["vport"], 1);
    o.s(0, "VPORT").s(5, &hs["vportRec"]).s(330, &hs["vport"]).s(100, "AcDbSymbolTableRecord").s(100, "AcDbViewportTableRecord").s(2, "*Active").i(70, 0);
    o.n(10, 0.0).n(20, 0.0).n(11, 1.0).n(21, 1.0).n(12, (ext.min_x + ext.max_x) / 2.0).n(22, (ext.min_y + ext.max_y) / 2.0).n(13, 0.0).n(23, 0.0).n(14, 10.0).n(24, 10.0).n(15, 10.0).n(25, 10.0);
    o.n(16, 0.0).n(26, 0.0).n(36, 1.0).n(17, 0.0).n(27, 0.0).n(37, 0.0).n(40, (ext.max_y - ext.min_y).max(1.0) * 1.2).n(41, 1.5).n(42, 50.0).n(43, 0.0).n(44, 0.0).n(50, 0.0).n(51, 0.0);
    o.i(71, 0).i(72, 100).i(73, 1).i(74, 3).i(75, 0).i(76, 1).i(77, 0).i(78, 0);
    o.s(0, "ENDTAB");

    table_start(&mut o, "LTYPE", &hs["ltype"], 2 + ltype_names.len());
    let ltype_ent = |o: &mut Out, name: &str, desc: &str, pattern: &[f64]| {
        o.s(0, "LTYPE").s(5, &ltype_handle[name]).s(330, &hs["ltype"]).s(100, "AcDbSymbolTableRecord").s(100, "AcDbLinetypeTableRecord").s(2, name).i(70, 0).s(3, desc).i(72, 65).i(73, pattern.len() as i64);
        o.n(40, pattern.iter().map(|v| v.abs()).sum());
        for v in pattern {
            o.n(49, *v).i(74, 0);
        }
    };
    ltype_ent(&mut o, "ByBlock", "", &[]);
    ltype_ent(&mut o, "ByLayer", "", &[]);
    for l in &ltype_names {
        ltype_ent(&mut o, l.name, l.desc, l.pattern);
    }
    o.s(0, "ENDTAB");

    table_start(&mut o, "LAYER", &hs["layer"], layers.len());
    for (i, l) in doc.layers.iter().enumerate() {
        let (aci, tc) = match l.color {
            Color::Aci(n) => (if n == 0 { 7 } else { n }, None),
            Color::Rgb(r, g, b) => (rgb_to_aci([r, g, b]), Some(((r as i64) << 16) | ((g as i64) << 8) | b as i64)),
            _ => (7, None),
        };
        o.s(0, "LAYER").s(5, &layer_handle[i]).s(330, &hs["layer"]).s(100, "AcDbSymbolTableRecord").s(100, "AcDbLayerTableRecord").s(2, &l.name);
        o.i(70, (l.frozen as i64) | if l.locked { 4 } else { 0 }).i(62, if l.on { aci as i64 } else { -(aci as i64) });
        if let Some(tc) = tc {
            o.i(420, tc);
        }
        let lt = if linetype(&l.ltype).is_some() { l.ltype.as_str() } else { "Continuous" };
        o.s(6, lt).i(290, l.plot as i64).i(370, l.lw as i64);
    }
    o.s(0, "ENDTAB");

    table_start(&mut o, "STYLE", &hs["style"], 1);
    o.s(0, "STYLE").s(5, &hs["styleRec"]).s(330, &hs["style"]).s(100, "AcDbSymbolTableRecord").s(100, "AcDbTextStyleTableRecord").s(2, "Standard").i(70, 0).n(40, 0.0).n(41, 1.0).n(50, 0.0).i(71, 0).n(42, 2.5).s(3, "arial.ttf").s(4, "");
    o.s(0, "ENDTAB");
    table_start(&mut o, "VIEW", &hs["view"], 0);
    o.s(0, "ENDTAB");
    table_start(&mut o, "UCS", &hs["ucs"], 0);
    o.s(0, "ENDTAB");
    table_start(&mut o, "APPID", &hs["appid"], 2);
    o.s(0, "APPID").s(5, &hs["appidRec"]).s(330, &hs["appid"]).s(100, "AcDbSymbolTableRecord").s(100, "AcDbRegAppTableRecord").s(2, "ACAD").i(70, 0);
    let cf_app = w.h();
    o.s(0, "APPID").s(5, &cf_app).s(330, &hs["appid"]).s(100, "AcDbSymbolTableRecord").s(100, "AcDbRegAppTableRecord").s(2, CADFORGE_APP).i(70, 0);
    o.s(0, "ENDTAB");

    // every dimension style, Standard first (it has the reserved handle)
    let std_ds = st.dim_styles.get("Standard").unwrap_or(ds);
    let others: Vec<(&String, &crate::style::DimStyle)> = st.dim_styles.iter().filter(|(k, _)| k.as_str() != "Standard").collect();
    table_start(&mut o, "DIMSTYLE", &hs["dimstyle"], 1 + others.len());
    let dimstyle_rec = |o: &mut Out, hnd: &str, name: &str, d: &crate::style::DimStyle| {
        o.s(0, "DIMSTYLE").s(105, hnd).s(330, &hs["dimstyle"]).s(100, "AcDbSymbolTableRecord").s(100, "AcDbDimStyleTableRecord").s(2, name).i(70, 0);
        if !d.pre.is_empty() || !d.suf.is_empty() {
            o.s(3, &format!("{}<>{}", clean(&d.pre), clean(&d.suf)));
        }
        o.n(40, d.scale).n(41, d.asz).n(42, d.exo).n(43, 3.75).n(44, d.exe).n(140, d.txt).n(141, d.cen).n(144, d.lfac).n(147, d.gap).i(271, d.dec as i64).i(77, d.tad as i64).i(78, d.zin as i64);
    };
    dimstyle_rec(&mut o, &hs["dimstyleRec"], "Standard", std_ds);
    for (name, d) in others {
        let h = w.h();
        dimstyle_rec(&mut o, &h, name, d);
    }
    o.s(0, "ENDTAB");
    let br_count = 1 + sheets.len() + user_blocks.len() + w.dim_blocks.len();
    table_start(&mut o, "BLOCK_RECORD", &hs["blockRecord"], br_count);
    let rec = |o: &mut Out, name: &str, hnd: &str, layout: Option<&str>| {
        o.s(0, "BLOCK_RECORD").s(5, hnd).s(330, &hs["blockRecord"]).s(100, "AcDbSymbolTableRecord").s(100, "AcDbBlockTableRecord").s(2, name).i(70, 0).i(280, 1).i(281, 0);
        if let Some(l) = layout {
            o.s(340, l);
        }
    };
    rec(&mut o, "*Model_Space", &hs["msRec"], Some(&model_layout));
    for (i, h) in sheet_h.iter().enumerate() {
        rec(&mut o, &sheet_block(i), &h.rec, Some(&h.layout));
    }
    for b in &user_blocks {
        rec(&mut o, &b.name, &w.block_rec[&b.name].0, None);
    }
    for d in &w.dim_blocks {
        rec(&mut o, &d.name, &d.rec, None);
    }
    o.s(0, "ENDTAB");
    o.s(0, "ENDSEC");

    // ---------- BLOCKS
    o.s(0, "SECTION").s(2, "BLOCKS");
    let block = |o: &mut Out, name: &str, hnd: &str, end_h: &str, owner: &str, body: &[String], base: Pt, layer: &str| {
        let anon = name.starts_with('*') && !name.starts_with("*Model_Space") && !name.starts_with("*Paper_Space");
        o.s(0, "BLOCK").s(5, hnd).s(330, owner).s(100, "AcDbEntity").s(8, layer).s(100, "AcDbBlockBegin").s(2, name).i(70, anon as i64).pt(base, 0).s(3, name).s(1, "");
        o.lines.extend(body.iter().cloned());
        o.s(0, "ENDBLK").s(5, end_h).s(330, owner).s(100, "AcDbEntity").s(8, layer).s(100, "AcDbBlockEnd");
    };
    block(&mut o, "*Model_Space", &hs["msBlock"], &hs["msEnd"], &hs["msRec"], &[], pt(0.0, 0.0), "0");
    for (i, h) in sheet_h.iter().enumerate() {
        block(&mut o, &sheet_block(i), &h.blk, &h.end, &h.rec, &sheet_lines[i], pt(0.0, 0.0), "0");
    }
    for (b, body) in user_blocks.iter().zip(&block_outs) {
        let (r, blk, end) = &w.block_rec[&b.name];
        block(&mut o, &b.name, blk, end, r, body, b.base, "0");
    }
    for d in &w.dim_blocks {
        block(&mut o, &d.name, &d.blk, &d.end, &d.rec, &d.lines, pt(0.0, 0.0), &d.layer);
    }
    o.s(0, "ENDSEC");

    // ---------- ENTITIES
    o.s(0, "SECTION").s(2, "ENTITIES");
    o.lines.extend(model_out);
    o.s(0, "ENDSEC");

    // ---------- OBJECTS (root dictionary, groups and the layout dictionary)
    o.s(0, "SECTION").s(2, "OBJECTS");
    let dict_images = w.h();
    o.s(0, "DICTIONARY").s(5, &hs["dictRoot"]).i(330, 0).s(100, "AcDbDictionary").i(281, 1).s(3, "ACAD_GROUP").s(350, &hs["dictGroup"]).s(3, "ACAD_LAYOUT").s(350, &dict_layout);
    if !w.image_defs.is_empty() {
        o.s(3, "ACAD_IMAGE_DICT").s(350, &dict_images);
    }
    let extras = cadforge_extras(doc, &w.handle_of).map(|json| (w.h(), json));
    if let Some((h, _)) = &extras {
        o.s(3, CADFORGE_APP).s(350, h);
    }
    if let Some((h, json)) = &extras {
        o.s(0, "XRECORD").s(5, h).s(102, "{ACAD_REACTORS").s(330, &hs["dictRoot"]).s(102, "}").s(330, &hs["dictRoot"]).s(100, "AcDbXrecord").i(280, 1);
        for chunk in xdata_chunks(json) {
            o.s(1, chunk);
        }
    }
    o.s(0, "DICTIONARY").s(5, &hs["dictGroup"]).s(330, &hs["dictRoot"]).s(100, "AcDbDictionary").i(281, 1);
    let groups: Vec<&(String, String, Vec<String>)> = w.groups.iter().filter(|g| !g.2.is_empty()).collect();
    for (id, h, _) in &groups {
        o.s(3, &enc(id)).s(350, h);
    }
    for (id, h, members) in &groups {
        // CadForge's own groups are unnamed (*A...), like AutoCAD's anonymous ones
        o.s(0, "GROUP").s(5, h).s(102, "{ACAD_REACTORS").s(330, &hs["dictGroup"]).s(102, "}").s(330, &hs["dictGroup"]).s(100, "AcDbGroup").s(300, "").i(70, id.starts_with('*') as i64).i(71, 1);
        for m in members {
            o.s(340, m);
        }
    }
    o.s(0, "DICTIONARY").s(5, &dict_layout).s(330, &hs["dictRoot"]).s(100, "AcDbDictionary").i(281, 1);
    o.s(3, "Model").s(350, &model_layout);
    for ((name, _, _), h) in sheets.iter().zip(&sheet_h) {
        o.s(3, &enc(name)).s(350, &h.layout);
    }
    let layout_obj = |o: &mut Out, hnd: &str, name: &str, tab: i64, block_rec: &str, paper: Option<&Paper>| {
        o.s(0, "LAYOUT").s(5, hnd).s(330, &dict_layout).s(100, "AcDbPlotSettings").s(1, "").s(2, "none_device").s(4, paper.map_or("", |p| if p.name == "Custom" { "" } else { p.name.as_str() }));
        let (pw, ph) = paper.map_or((0.0, 0.0), |p| if p.landscape { (p.w.max(p.h), p.w.min(p.h)) } else { (p.w.min(p.h), p.w.max(p.h)) });
        o.s(6, "").n(40, 0.0).n(41, 0.0).n(42, 0.0).n(43, 0.0).n(44, pw).n(45, ph).n(46, 0.0).n(47, 0.0).n(48, 0.0).n(49, 0.0).n(140, 0.0).n(141, 0.0).n(142, 1.0).n(143, 1.0);
        o.i(70, 688).i(72, 1).i(73, 0).i(74, 5).s(7, "").i(75, 16).n(147, 1.0).n(148, 0.0).n(149, 0.0);
        o.s(100, "AcDbLayout").s(1, name).i(70, 1).i(71, tab).n(10, 0.0).n(20, 0.0).n(11, pw).n(21, ph).n(12, 0.0).n(22, 0.0).n(32, 0.0);
        o.n(14, 0.0).n(24, 0.0).n(34, 0.0).n(15, pw).n(25, ph).n(35, 0.0).n(146, 0.0).n(13, 0.0).n(23, 0.0).n(33, 0.0);
        o.n(16, 1.0).n(26, 0.0).n(36, 0.0).n(17, 0.0).n(27, 1.0).n(37, 0.0).i(76, 0).s(330, block_rec);
    };
    if !w.image_defs.is_empty() {
        o.s(0, "DICTIONARY").s(5, &dict_images).s(330, &hs["dictRoot"]).s(100, "AcDbDictionary").i(281, 1);
        for (name, def_h, _) in &w.image_defs {
            o.s(3, &enc(name)).s(350, def_h);
        }
        for (name, def_h, reactors) in &w.image_defs {
            let d = &doc.images[name];
            o.s(0, "IMAGEDEF").s(5, def_h).s(102, "{ACAD_REACTORS").s(330, &dict_images);
            for (r, _) in reactors {
                o.s(330, r);
            }
            o.s(102, "}").s(330, &dict_images).s(100, "AcDbRasterImageDef").i(90, 0).s(1, &enc(&d.path)).n(10, d.w as f64).n(20, d.h as f64).n(11, 1.0).n(21, 1.0).i(280, 1).i(281, 0);
            for (r, ent) in reactors {
                o.s(0, "IMAGEDEF_REACTOR").s(5, r).s(330, ent).s(100, "AcDbRasterImageDefReactor").i(90, 2).s(330, ent);
            }
        }
    }
    layout_obj(&mut o, &model_layout, "Model", 0, &hs["msRec"], None);
    for (i, ((name, paper, _), h)) in sheets.iter().zip(&sheet_h).enumerate() {
        layout_obj(&mut o, &h.layout, &enc(name), i as i64 + 1, &h.rec, Some(paper));
    }
    o.s(0, "ENDSEC").s(0, "EOF");

    o.text().replacen("__HANDSEED__", &format!("{:X}", w.handle), 1)
}
