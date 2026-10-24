//! Turn the objects of a DWG file into DXF text for the DXF reader: tables (linetypes, layers, text styles, block
//! records), blocks (block definitions and every paper-space layout), model-space entities, and the LAYOUT and IMAGEDEF
//! objects. References are resolved here, once every object is known.

use super::header::Header;
use super::objects::{Col, EntCommon, Info, Lt, Obj, V};
use std::collections::HashMap;
use std::fmt::Write;

struct W {
    s: String,
}

impl W {
    fn g(&mut self, code: i32, v: impl std::fmt::Display) {
        let _ = write!(self.s, "{code}\n{v}\n");
    }
    fn text(&mut self, code: i32, v: &str, mtext: bool) {
        let clean = if mtext {
            v.replace("\r\n", "\\P").replace(['\n', '\r'], "\\P")
        } else {
            v.replace(['\n', '\r'], " ")
        };
        self.g(code, clean);
    }
}

fn hex(h: u64) -> String {
    format!("{h:X}")
}

pub fn emit(objs: &[Obj], hdr: &Header) -> String {
    let mut w = W {
        s: String::with_capacity(objs.len() * 200),
    };
    // ---- names by handle
    let mut names: HashMap<u64, String> = HashMap::new();
    let block_name: HashMap<u64, String> = objs
        .iter()
        .filter_map(|o| {
            if let Info::Block { name, .. } = &o.info {
                Some((o.handle, name.clone()))
            } else {
                None
            }
        })
        .collect();
    for o in objs {
        let n = match &o.info {
            Info::Layer { name, .. }
            | Info::Ltype { name, .. }
            | Info::Style { name, .. }
            | Info::DimStyle { name, .. }
            | Info::AppId(name) => name.clone(),
            Info::BlockRecord { name, begin, .. } => block_name
                .get(begin)
                .filter(|n| !n.is_empty())
                .cloned()
                .unwrap_or_else(|| name.clone()),
            _ => continue,
        };
        names.insert(o.handle, n);
    }
    let name_of = |h: u64| names.get(&h).cloned().unwrap_or_default();
    let book: HashMap<u64, Col> = objs
        .iter()
        .filter_map(|o| {
            if let Info::DbColor(c) = o.info {
                Some((o.handle, c))
            } else {
                None
            }
        })
        .collect();
    let records: Vec<&Obj> = objs
        .iter()
        .filter(|o| matches!(o.info, Info::BlockRecord { .. }))
        .collect();
    let find_br = |want: &str| {
        records
            .iter()
            .find(|o| name_of(o.handle).eq_ignore_ascii_case(want))
            .map(|o| o.handle)
    };
    let model = find_br("*Model_Space").unwrap_or(u64::MAX);
    let paper = find_br("*Paper_Space").unwrap_or(u64::MAX - 1);

    // ---- who owns what
    let mut children: HashMap<u64, Vec<usize>> = HashMap::new(); // owner -> entity indices, in handle order
    let mut model_ents: Vec<usize> = vec![];
    for (i, o) in objs.iter().enumerate() {
        let Some(e) = &o.ent else { continue };
        if matches!(o.ty, "BLOCK" | "ENDBLK") {
            continue;
        }
        match e.mode {
            2 => model_ents.push(i),
            1 => children.entry(paper).or_default().push(i),
            _ if o.owner == model => model_ents.push(i),
            _ => children.entry(o.owner).or_default().push(i),
        }
    }
    let by_handle = |v: &mut Vec<usize>| v.sort_by_key(|&i| objs[i].handle);
    by_handle(&mut model_ents);
    for v in children.values_mut() {
        by_handle(v);
    }

    // ---- header
    w.g(0, "SECTION");
    w.g(2, "HEADER");
    w.g(9, "$ACADVER");
    w.g(1, "AC1015");
    for (k, v) in [
        ("$LTSCALE", hdr.ltscale),
        ("$TEXTSIZE", hdr.textsize),
        ("$PDSIZE", hdr.pdsize),
        ("$DIMSCALE", hdr.dimscale),
        ("$DIMASZ", hdr.dimasz),
        ("$DIMEXO", hdr.dimexo),
        ("$DIMEXE", hdr.dimexe),
        ("$DIMTXT", hdr.dimtxt),
        ("$DIMCEN", hdr.dimcen),
        ("$DIMGAP", hdr.dimgap),
    ] {
        if v.is_finite() {
            w.g(9, k);
            w.g(40, v);
        }
    }
    w.g(9, "$PDMODE");
    w.g(70, hdr.pdmode);
    w.g(9, "$LUPREC");
    w.g(70, hdr.luprec);
    if let Some(d) = hdr.dimdec {
        w.g(9, "$DIMDEC");
        w.g(70, d);
    }
    if let Some(u) = hdr.insunits {
        w.g(9, "$INSUNITS");
        w.g(70, u);
    }
    if let Some(n) = names.get(&hdr.clayer) {
        w.g(9, "$CLAYER");
        w.text(8, n, false);
    }
    w.g(0, "ENDSEC");

    // ---- tables
    w.g(0, "SECTION");
    w.g(2, "TABLES");
    w.g(0, "TABLE");
    w.g(2, "LTYPE");
    for o in objs {
        if let Info::Ltype {
            name,
            desc,
            dashes,
            complex,
            total,
        } = &o.info
        {
            w.g(0, "LTYPE");
            w.g(5, hex(o.handle));
            w.text(2, name, false);
            w.g(70, 0);
            w.text(3, desc, false);
            w.g(72, 65);
            w.g(73, dashes.len());
            w.g(40, total);
            for d in dashes {
                w.g(49, d);
                w.g(74, if *complex { 1 } else { 0 });
            }
        }
    }
    w.g(0, "ENDTAB");
    w.g(0, "TABLE");
    w.g(2, "LAYER");
    for o in objs {
        if let Info::Layer {
            name,
            color,
            frozen,
            off,
            locked,
            plot,
            lw,
            lt,
        } = &o.info
        {
            w.g(0, "LAYER");
            w.g(5, hex(o.handle));
            w.text(2, name, false);
            w.g(70, (*frozen as i32) | ((*locked as i32) << 2));
            let (aci, rgb) = match color {
                Col::Aci(i) => (*i as i32, None),
                Col::Rgb(c) => {
                    let [r, g, b] = [(c >> 16) as u8, (c >> 8) as u8, *c as u8];
                    (crate::style::rgb_to_aci([r, g, b]) as i32, Some(*c))
                }
                _ => (7, None),
            };
            w.g(62, if *off { -aci.max(1) } else { aci.max(1) });
            if let Some(c) = rgb {
                w.g(420, c);
            }
            let ltn = names
                .get(lt)
                .cloned()
                .unwrap_or_else(|| "Continuous".into());
            w.text(6, &ltn, false);
            w.g(370, if *lw < 0 { 25 } else { *lw as i32 });
            w.g(290, *plot as i32);
        }
    }
    w.g(0, "ENDTAB");
    w.g(0, "TABLE");
    w.g(2, "STYLE");
    for o in objs {
        if let Info::Style {
            name,
            height,
            width,
            font,
            bigfont,
        } = &o.info
        {
            w.g(0, "STYLE");
            w.g(5, hex(o.handle));
            w.text(2, name, false);
            w.g(70, 0);
            w.g(40, height);
            w.g(41, width);
            w.text(3, font, false);
            w.text(4, bigfont, false);
        }
    }
    w.g(0, "ENDTAB");
    w.g(0, "TABLE");
    w.g(2, "BLOCK_RECORD");
    for o in &records {
        if let Info::BlockRecord { layout, .. } = &o.info {
            w.g(0, "BLOCK_RECORD");
            w.g(5, hex(o.handle));
            w.text(2, &name_of(o.handle), false);
            if *layout != 0 {
                w.g(340, hex(*layout));
            }
        }
    }
    w.g(0, "ENDTAB");
    w.g(0, "ENDSEC");

    // ---- blocks: definitions and paper-space layouts
    let ctx = Ctx {
        objs,
        names: &names,
        children: &children,
        book: &book,
        arrow: hdr.dimasz
            * if hdr.dimscale > 0.0 {
                hdr.dimscale
            } else {
                1.0
            },
    };
    w.g(0, "SECTION");
    w.g(2, "BLOCKS");
    for o in &records {
        let Info::BlockRecord { base, .. } = &o.info else {
            continue;
        };
        if o.handle == model {
            continue;
        }
        let bname = name_of(o.handle);
        w.g(0, "BLOCK");
        w.g(8, "0");
        w.text(2, &bname, false);
        w.g(70, 0);
        w.g(10, base[0]);
        w.g(20, base[1]);
        w.g(30, base[2]);
        // the first viewport of a layout is the sheet's own (id 1)
        let mut vp_id = 0;
        if let Some(list) = children.get(&o.handle) {
            for &i in list {
                if objs[i].ty == "VIEWPORT" {
                    vp_id += 1;
                }
                ctx.entity(&mut w, i, if objs[i].ty == "VIEWPORT" { vp_id } else { 0 });
            }
        }
        w.g(0, "ENDBLK");
        w.g(8, "0");
    }
    w.g(0, "ENDSEC");

    // ---- model space
    w.g(0, "SECTION");
    w.g(2, "ENTITIES");
    let mut vp_id = 0;
    for &i in &model_ents {
        if objs[i].ty == "VIEWPORT" {
            vp_id += 1;
        }
        ctx.entity(&mut w, i, vp_id);
    }
    w.g(0, "ENDSEC");

    // ---- objects
    w.g(0, "SECTION");
    w.g(2, "OBJECTS");
    for o in objs {
        match &o.info {
            Info::Layout {
                name,
                tab,
                block,
                paper_w,
                paper_h,
                units,
                rot,
                paper,
            } => {
                w.g(0, "LAYOUT");
                w.g(5, hex(o.handle));
                w.g(100, "AcDbPlotSettings");
                w.text(4, paper, false);
                w.g(44, paper_w);
                w.g(45, paper_h);
                w.g(72, units);
                w.g(73, rot);
                w.g(100, "AcDbLayout");
                w.text(1, name, false);
                w.g(71, tab);
                w.g(330, hex(*block));
            }
            Info::ImageDef { path, w: iw, h: ih } => {
                w.g(0, "IMAGEDEF");
                w.g(5, hex(o.handle));
                w.text(1, path, false);
                w.g(10, iw);
                w.g(20, ih);
            }
            // groups, with the dictionary entries that name them
            Info::Dict(entries) => {
                let groups: Vec<&(String, u64)> = entries
                    .iter()
                    .filter(|(_, h)| {
                        objs.binary_search_by_key(h, |x| x.handle)
                            .is_ok_and(|k| objs[k].ty == "GROUP")
                    })
                    .collect();
                if !groups.is_empty() {
                    w.g(0, "DICTIONARY");
                    w.g(5, hex(o.handle));
                    for (name, h) in groups {
                        w.text(3, name, false);
                        w.g(350, hex(*h));
                    }
                }
            }
            Info::Group(members) => {
                w.g(0, "GROUP");
                w.g(5, hex(o.handle));
                for m in members {
                    w.g(340, hex(*m));
                }
            }
            _ => {}
        }
    }
    w.g(0, "ENDSEC");
    w.g(0, "EOF");
    w.s
}

struct Ctx<'a> {
    objs: &'a [Obj],
    names: &'a HashMap<u64, String>,
    children: &'a HashMap<u64, Vec<usize>>,
    book: &'a HashMap<u64, Col>,
    /// Arrowhead size (DIMASZ x DIMSCALE), the length of leader hook lines.
    arrow: f64,
}

impl Ctx<'_> {
    fn common(&self, w: &mut W, o: &Obj, e: &EntCommon) {
        w.g(5, hex(o.handle));
        w.text(
            8,
            self.names.get(&e.layer).map_or("0", |s| s.as_str()),
            false,
        );
        match e.lt {
            Lt::ByLayer => {}
            Lt::ByBlock => w.g(6, "BYBLOCK"),
            Lt::Continuous => w.g(6, "Continuous"),
            Lt::H(h) => {
                if let Some(n) = self.names.get(&h) {
                    w.text(6, n, false);
                }
            }
        }
        let color = match e.color {
            Col::Book(h) => self.book.get(&h).copied().unwrap_or(Col::ByBlock),
            c => c,
        };
        match color {
            Col::ByLayer | Col::Book(_) => {}
            Col::ByBlock => w.g(62, 0),
            Col::Aci(i) => w.g(62, i),
            Col::Rgb(c) => {
                w.g(
                    62,
                    crate::style::rgb_to_aci([(c >> 16) as u8, (c >> 8) as u8, c as u8]),
                );
                w.g(420, c);
            }
        }
        if e.lw >= 0 || e.lw == -2 {
            w.g(370, e.lw);
        }
        if e.ltscale.is_finite() && (e.ltscale - 1.0).abs() > 1e-12 && e.ltscale > 0.0 {
            w.g(48, e.ltscale);
        }
        if e.invisible {
            w.g(60, 1);
        }
    }

    fn groups(&self, w: &mut W, o: &Obj) {
        let mtext = o.ty == "MTEXT";
        for (c, v) in &o.g {
            match v {
                V::I(x) => w.g(*c, x),
                V::F(x) => w.g(*c, if x.is_finite() { *x } else { 0.0 }),
                V::S(s) => w.text(*c, s, mtext),
                V::Name(h) => w.text(*c, self.names.get(h).map_or("", |s| s.as_str()), false),
                V::Hex(h) => w.g(*c, hex(*h)),
            }
        }
    }

    /// A leader. DWG leaders keep neither the hook line nor (from 2010) the text height; AutoCAD works them out from
    /// the attached MTEXT, and so do we: the hook is an arrow-length horizontal segment ending at the last vertex,
    /// added when the last segment is steeper than 15 degrees.
    fn leader(&self, w: &mut W, o: &Obj) {
        let f = |c: i32, g: &[(i32, V)]| {
            g.iter().find_map(|(k, v)| {
                if *k == c {
                    if let V::F(x) = v {
                        Some(*x)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        };
        let annot = o.g.iter().find_map(|(c, v)| {
            if *c == 340 {
                if let V::Hex(h) = v {
                    Some(*h)
                } else {
                    None
                }
            } else {
                None
            }
        });
        let mtext = annot
            .and_then(|a| self.objs.binary_search_by_key(&a, |x| x.handle).ok())
            .map(|k| &self.objs[k])
            .filter(|m| m.ty == "MTEXT");
        let mut pts = vec![];
        let mut it = o.g.iter();
        while let Some((c, v)) = it.next() {
            if let (10, V::F(x)) = (c, v) {
                let y = it
                    .next()
                    .and_then(|(_, v)| if let V::F(y) = v { Some(*y) } else { None })
                    .unwrap_or(0.0);
                let z = it
                    .next()
                    .and_then(|(_, v)| if let V::F(z) = v { Some(*z) } else { None })
                    .unwrap_or(0.0);
                pts.push([*x, y, z]);
            }
        }
        let xd = [f(211, &o.g).unwrap_or(1.0), f(221, &o.g).unwrap_or(0.0)];
        // CadForge's own leaders put their text just right of the last vertex (0.4 text heights, middle left) and
        // draw no hook; those read back as they were written
        let cadforge = mtext.zip(pts.last()).is_some_and(|(m, l)| {
            let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0);
            let h = f(40, &o.g).or_else(|| f(40, &m.g)).unwrap_or(0.0);
            let attach = m.g.iter().find_map(|(c, v)| {
                if *c == 71 {
                    if let V::I(x) = v {
                        Some(*x)
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
            attach == Some(4)
                && close(f(10, &m.g).unwrap_or(f64::NAN), l[0] + 0.4 * h)
                && close(f(20, &m.g).unwrap_or(f64::NAN), l[1])
        });
        let mut hook = false;
        if mtext.is_some() && pts.len() >= 2 && !cadforge {
            let (a, b) = (pts[pts.len() - 2], pts[pts.len() - 1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let along = dx * xd[0] + dy * xd[1];
            let across = -dx * xd[1] + dy * xd[0];
            if across.abs() > along.abs() * 15f64.to_radians().tan() {
                let style = o.g.iter().find_map(|(c, v)| {
                    if *c == 3 {
                        if let V::Name(h) = v {
                            Some(*h)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });
                let len = style
                    .and_then(|h| self.objs.binary_search_by_key(&h, |x| x.handle).ok())
                    .and_then(|k| {
                        if let Info::DimStyle { arrow, .. } = &self.objs[k].info {
                            *arrow
                        } else {
                            None
                        }
                    })
                    .unwrap_or(self.arrow);
                let s = if along < 0.0 { -1.0 } else { 1.0 };
                pts.insert(
                    pts.len() - 1,
                    [b[0] - xd[0] * len * s, b[1] - xd[1] * len * s, b[2]],
                );
                hook = true;
            }
        }
        for (c, v) in &o.g {
            match (c, v) {
                (10 | 20 | 30, _) => {}
                (76, _) => {
                    w.g(75, hook as i32);
                    if f(40, &o.g).is_none() {
                        if let Some(h) = mtext
                            .and_then(|m| f(43, &m.g).filter(|h| *h > 0.0).or_else(|| f(40, &m.g)))
                        {
                            w.g(40, h);
                        }
                    }
                    w.g(76, pts.len() as i32);
                    for p in &pts {
                        w.g(10, p[0]);
                        w.g(20, p[1]);
                        w.g(30, p[2]);
                    }
                }
                (_, V::I(x)) => w.g(*c, x),
                (_, V::F(x)) => w.g(*c, if x.is_finite() { *x } else { 0.0 }),
                (_, V::Hex(h)) => w.g(*c, hex(*h)),
                (_, V::S(s)) => w.text(*c, s, false),
                (_, V::Name(h)) => w.text(*c, self.names.get(h).map_or("", |s| s.as_str()), false),
            }
        }
    }

    fn entity(&self, w: &mut W, i: usize, vp_id: i32) {
        let o = &self.objs[i];
        let Some(e) = &o.ent else { return };
        if matches!(o.ty, "VERTEX" | "ATTRIB" | "SEQEND") {
            return; // written with their polyline / insert
        }
        w.g(0, o.ty);
        self.common(w, o, e);
        if o.ty == "VIEWPORT" {
            w.g(69, vp_id);
        }
        if o.ty == "LEADER" {
            self.leader(w, o);
        } else {
            self.groups(w, o);
        }
        let follows = o.g.iter().any(|(c, v)| *c == 66 && matches!(v, V::I(1)));
        if follows {
            let want = if o.ty == "POLYLINE" {
                "VERTEX"
            } else {
                "ATTRIB"
            };
            if let Some(kids) = self.children.get(&o.handle) {
                for &k in kids {
                    let c = &self.objs[k];
                    if c.ty != want {
                        continue;
                    }
                    let Some(ce) = &c.ent else { continue };
                    w.g(0, c.ty);
                    self.common(w, c, ce);
                    self.groups(w, c);
                }
            }
            w.g(0, "SEQEND");
            w.g(5, "0");
            w.text(
                8,
                self.names.get(&e.layer).map_or("0", |s| s.as_str()),
                false,
            );
        }
    }
}
