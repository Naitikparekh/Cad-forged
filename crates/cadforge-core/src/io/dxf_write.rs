//! DXF writer (AutoCAD R2000 / AC1015), port of `src/io/dxf-write.js`. Output is validated against ezdxf by the tests
//! in `tests/dxf.rs`.
use crate::dim::{arrow_lines, arrow_polygon, dim_geometry, DimType};
use crate::drawing::Drawing;
use crate::entities::*;
use crate::geom::*;
use crate::hatch::pattern;
use crate::style::*;
use crate::text::{HJust, VJust};
use std::collections::HashMap;

/// Format a number the way the JS writer does (15 significant digits, no exponent).
pub fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let r: f64 = format!("{:.14e}", v).parse().unwrap_or(v);
    format!("{}", r + 0.0)
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
}

impl<'a> W<'a> {
    fn h(&mut self) -> String {
        let s = format!("{:X}", self.handle);
        self.handle += 1;
        s
    }

    fn common(&mut self, o: &mut EOut, ty: &str, p: &Props, sub: Option<&str>, layer: Option<&str>) {
        let hnd = self.h();
        o.out.s(0, ty).s(5, &hnd).s(330, &o.owner.clone()).s(100, "AcDbEntity");
        if o.block_name == "*Paper_Space" {
            o.out.i(67, 1);
        }
        let layer = layer.unwrap_or(p.layer);
        o.out.s(8, if layer.is_empty() { "0" } else { layer });
        if let Some(lt) = p.ltype {
            let lt = if lt == "BYBLOCK" { "ByBlock" } else { lt };
            if lt == "ByBlock" || LINETYPES.iter().any(|l| l.name == lt) {
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
            if v.b != 0.0 {
                o.out.n(42, v.b);
            }
        }
    }

    fn write(&mut self, o: &mut EOut, e: &Entity) {
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
                o.out.s(2, &ins.name).pt(ins.p, 0).n(41, ins.sx).n(42, ins.sy).n(43, 1.0).n(50, deg(ins.rot));
            }
            Kind::Leader { pts, s, h } => {
                self.common(o, "LEADER", &props, Some("AcDbLeader"), None);
                o.out.s(3, "Standard").i(71, 1).i(72, 0).i(73, 3).i(74, 1).i(75, 0).n(40, *h).n(41, 0.0).i(76, pts.len() as i64);
                for p in pts {
                    o.out.pt(*p, 0);
                }
                if !s.is_empty() {
                    if let Some(last) = pts.last() {
                        self.text(o, pt(last.x + h * 0.4, last.y), *h, s, 0.0, HJust::Left, VJust::Middle, &props, None);
                    }
                }
            }
            Kind::Hatch(ha) => {
                let pat = pattern(&ha.pattern).or_else(|| pattern("ANSI31")).expect("ANSI31 exists");
                let solid_fill = pat.solid;
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
            }
            Kind::Dim(d) => self.write_dim(o, e, d),
            Kind::Viewport(_) => {}
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
        if d.kind == DimType::Ang {
            // angular dimensions are written as plain geometry
            emit(self, o);
            return;
        }
        let rec = DimBlock { name: format!("*D{}", self.dim_blocks.len() + 1), rec: self.h(), blk: self.h(), end: self.h(), layer: layer.clone(), lines: vec![] };
        let mut inner = EOut { out: Out::default(), owner: rec.rec.clone(), block_name: o.block_name.clone() };
        emit(self, &mut inner);
        let rec = DimBlock { lines: inner.out.lines, ..rec };
        let ty = match d.kind {
            DimType::Lin => 0,
            DimType::Ali => 1,
            DimType::Dia => 3,
            DimType::Rad => 4,
            DimType::Ang => 0,
        };
        let zero = pt(0.0, 0.0);
        let def = match d.kind {
            DimType::Rad => d.c.unwrap_or(zero),
            DimType::Dia => {
                let (c, p) = (d.c.unwrap_or(zero), d.pt.unwrap_or(zero));
                pt(2.0 * c.x - p.x, 2.0 * c.y - p.y)
            }
            _ => d.dp.unwrap_or(zero),
        };
        let name = rec.name.clone();
        self.dim_blocks.push(rec);
        self.common(o, "DIMENSION", &Props::of(e), Some("AcDbDimension"), None);
        o.out.s(2, &name).pt(def, 0);
        o.out.pt(geo.text.p, 1).i(70, 32 | ty).i(71, 5).s(3, "Standard");
        if let Some(t) = &d.txt {
            if !t.is_empty() {
                o.out.s(1, t);
            }
        }
        match d.kind {
            DimType::Lin | DimType::Ali => {
                o.out.s(100, "AcDbAlignedDimension").pt(d.p1.unwrap_or(zero), 3).pt(d.p2.unwrap_or(zero), 4);
                if d.kind == DimType::Lin {
                    o.out.n(50, deg(d.ang.unwrap_or(0.0))).s(100, "AcDbRotatedDimension");
                }
            }
            _ => {
                o.out.s(100, if d.kind == DimType::Dia { "AcDbDiametricDimension" } else { "AcDbRadialDimension" }).pt(d.pt.unwrap_or(zero), 5).n(40, 0.0);
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

/// Serialise a drawing to DXF text.
pub fn write_dxf(doc: &Drawing) -> String {
    let mut w = W { doc, handle: 0x10, dim_blocks: vec![], block_rec: HashMap::new() };
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
    let ltype_names: Vec<&'static LineType> = used.iter().filter_map(|n| LINETYPES.iter().find(|l| l.name == n)).collect();
    let user_blocks: Vec<_> = doc.blocks.values().collect();

    // handles of fixed table objects
    let mut hs: HashMap<&str, String> = HashMap::new();
    for k in ["vport", "ltype", "layer", "style", "view", "ucs", "appid", "dimstyle", "blockRecord", "dictRoot", "dictGroup", "vportRec", "styleRec", "appidRec", "dimstyleRec", "msRec", "psRec", "msBlock", "msEnd", "psBlock", "psEnd"] {
        hs.insert(k, w.h());
    }
    for b in &user_blocks {
        let t = (w.h(), w.h(), w.h());
        w.block_rec.insert(b.name.clone(), t);
    }
    let layer_handle: Vec<String> = layers.iter().map(|_| w.h()).collect();
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
    let model_out = w.entity_out(&model_list, &hs["msRec"], "*Model_Space");
    let mut block_outs = vec![];
    for b in &user_blocks {
        let rec = w.block_rec[&b.name].0.clone();
        let list: Vec<&Entity> = b.entities.iter().collect();
        block_outs.push(w.entity_out(&list, &rec, &b.name));
    }
    let mut layout_lines: Vec<String> = vec![];
    for l in &doc.layouts {
        let list: Vec<&Entity> = l.entities.iter().filter(|e| !matches!(e.kind, Kind::Viewport(_))).collect();
        layout_lines.extend(w.entity_out(&list, &hs["psRec"], "*Paper_Space"));
    }

    // ---------- HEADER
    o.s(0, "SECTION").s(2, "HEADER");
    o.s(9, "$ACADVER").s(1, "AC1015").s(9, "$DWGCODEPAGE").s(3, "ANSI_1252");
    o.s(9, "$HANDSEED").s(5, "__HANDSEED__");
    o.s(9, "$INSBASE").pt(pt(0.0, 0.0), 0);
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
        let lt = if LINETYPES.iter().any(|t| t.name == l.ltype) { l.ltype.as_str() } else { "Continuous" };
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
    table_start(&mut o, "APPID", &hs["appid"], 1);
    o.s(0, "APPID").s(5, &hs["appidRec"]).s(330, &hs["appid"]).s(100, "AcDbSymbolTableRecord").s(100, "AcDbRegAppTableRecord").s(2, "ACAD").i(70, 0);
    o.s(0, "ENDTAB");

    table_start(&mut o, "DIMSTYLE", &hs["dimstyle"], 1);
    o.s(0, "DIMSTYLE").s(105, &hs["dimstyleRec"]).s(330, &hs["dimstyle"]).s(100, "AcDbSymbolTableRecord").s(100, "AcDbDimStyleTableRecord").s(2, "Standard").i(70, 0);
    o.n(40, ds.scale).n(41, ds.asz).n(42, ds.exo).n(43, 3.75).n(44, ds.exe).n(140, ds.txt).n(141, ds.cen).n(147, ds.gap).i(271, ds.dec as i64).i(77, ds.tad as i64);
    o.s(0, "ENDTAB");
    let br_count = 2 + user_blocks.len() + w.dim_blocks.len();
    table_start(&mut o, "BLOCK_RECORD", &hs["blockRecord"], br_count);
    let rec = |o: &mut Out, name: &str, hnd: &str| {
        o.s(0, "BLOCK_RECORD").s(5, hnd).s(330, &hs["blockRecord"]).s(100, "AcDbSymbolTableRecord").s(100, "AcDbBlockTableRecord").s(2, name).i(70, 0).i(280, 1).i(281, 0);
    };
    rec(&mut o, "*Model_Space", &hs["msRec"]);
    rec(&mut o, "*Paper_Space", &hs["psRec"]);
    for b in &user_blocks {
        rec(&mut o, &b.name, &w.block_rec[&b.name].0);
    }
    for d in &w.dim_blocks {
        rec(&mut o, &d.name, &d.rec);
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
    block(&mut o, "*Paper_Space", &hs["psBlock"], &hs["psEnd"], &hs["psRec"], &layout_lines, pt(0.0, 0.0), "0");
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

    // ---------- OBJECTS (root dictionary only)
    o.s(0, "SECTION").s(2, "OBJECTS");
    o.s(0, "DICTIONARY").s(5, &hs["dictRoot"]).i(330, 0).s(100, "AcDbDictionary").i(281, 1).s(3, "ACAD_GROUP").s(350, &hs["dictGroup"]);
    o.s(0, "DICTIONARY").s(5, &hs["dictGroup"]).s(330, &hs["dictRoot"]).s(100, "AcDbDictionary").i(281, 1);
    o.s(0, "ENDSEC").s(0, "EOF");

    o.text().replacen("__HANDSEED__", &format!("{:X}", w.handle), 1)
}
