//! Writing DWG: AutoCAD 2000 (AC1015) files, which every AutoCAD since 2000 and every DWG reader opens.
//!
//! The drawing is first written as DXF by [`crate::io::write_dxf`] (one mapping of CadForge's model to AutoCAD's
//! objects, shared by both formats); its records are then encoded as DWG objects, the way the reader turns DWG objects
//! into DXF groups. Added on the way are the objects a DWG must have that the DXF leaves out (the viewport entity
//! header control, the multiline style, the plot style dictionaries) and the links DWG keeps between objects (block
//! entity lists, attribute chains). Object layouts follow the Open Design Alliance specification as implemented by
//! ACadSharp (MIT License, Copyright (c) 2021 Albert Domenech).
//!
//! AutoCAD 2000 files store colours as palette indexes: true colours are written as their nearest index.

mod bitw;
mod dxfin;
mod header;

use crate::drawing::Drawing;
use bitw::{crc16, BitW};
use dxfin::{hex, Dxf, Rec};
use std::collections::HashMap;

/// Write a drawing as an AutoCAD 2000 DWG file.
pub fn write_dwg(doc: &Drawing) -> Result<Vec<u8>, String> {
    dxf_to_dwg(&crate::io::write_dxf(doc))
}

/// Encode CadForge's DXF output as an AutoCAD 2000 DWG file.
pub fn dxf_to_dwg(text: &str) -> Result<Vec<u8>, String> {
    let d = dxfin::parse(text);
    let mut w = Writer::new(&d)?;
    w.write_all()?;
    Ok(w.file())
}

fn rad(deg: f64) -> f64 {
    deg.to_radians()
}

/// A DWG lineweight index for a DXF lineweight (-1 ByLayer, -2 ByBlock, -3 default, else hundredths of a mm).
fn lw_index(v: i64) -> u8 {
    const W: [i64; 24] = [0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200, 211];
    match v {
        -1 => 29,
        -2 => 30,
        v if v < 0 => 31,
        v => W.iter().enumerate().min_by_key(|(_, w)| (*w - v).abs()).map_or(31, |(i, _)| i as u8),
    }
}

/// One object being encoded: its data stream and its handle stream (joined at the end).
struct O {
    m: BitW,
    h: BitW,
    size_at: usize,
}

impl O {
    fn new(ty: u16, handle: u64) -> O {
        let mut m = BitW::new();
        m.bs(ty as i16);
        let size_at = m.bits;
        m.rl(0); // size in bits of the data, patched in finish()
        m.h(0, handle);
        m.bs(0); // no extended data
        O { m, h: BitW::new(), size_at }
    }

    /// MS size, the object, CRC.
    fn finish(mut self) -> Vec<u8> {
        let bits = self.m.bits as u32;
        self.m.patch_rl(self.size_at, bits);
        self.m.append(&self.h);
        self.m.align();
        let n = self.m.buf.len() as u32;
        let mut out = if n >= 0x8000 {
            vec![(n & 0xFF) as u8, (((n >> 8) & 0x7F) | 0x80) as u8, ((n >> 15) & 0xFF) as u8, ((n >> 23) & 0xFF) as u8]
        } else {
            vec![(n & 0xFF) as u8, (n >> 8) as u8]
        };
        out.extend_from_slice(&self.m.buf);
        let crc = crc16(0xC0C1, &out);
        out.extend_from_slice(&crc.to_le_bytes());
        out
    }

    fn xref(&mut self) {
        self.m.bit(false);
        self.m.bs(0);
        self.m.bit(false);
    }
}

struct Class {
    dxf: &'static str,
    cpp: &'static str,
    app: &'static str,
    proxy: i16,
    entity: bool,
}

const CLASSES: [Class; 6] = [
    Class { dxf: "ACDBDICTIONARYWDFLT", cpp: "AcDbDictionaryWithDefault", app: "ObjectDBX Classes", proxy: 0, entity: false },
    Class { dxf: "ACDBPLACEHOLDER", cpp: "AcDbPlaceHolder", app: "ObjectDBX Classes", proxy: 0, entity: false },
    Class { dxf: "LAYOUT", cpp: "AcDbLayout", app: "ObjectDBX Classes", proxy: 0, entity: false },
    Class { dxf: "IMAGEDEF", cpp: "AcDbRasterImageDef", app: "ISM", proxy: 0, entity: false },
    Class { dxf: "IMAGEDEF_REACTOR", cpp: "AcDbRasterImageDefReactor", app: "ISM", proxy: 1, entity: false },
    Class { dxf: "IMAGE", cpp: "AcDbRasterImage", app: "ISM", proxy: 2175, entity: true },
];

/// An entity of a block with the ones that follow it (attributes and their SEQEND).
struct Ent<'a> {
    r: &'a Rec,
    kids: Vec<&'a Rec>,
    seqend: Option<&'a Rec>,
}

fn group(list: &[Rec]) -> Vec<Ent<'_>> {
    let mut out: Vec<Ent> = vec![];
    for r in list {
        match r.ty.as_str() {
            "ATTRIB" | "VERTEX" if !out.is_empty() => out.last_mut().expect("entity").kids.push(r),
            "SEQEND" if !out.is_empty() => out.last_mut().expect("entity").seqend = Some(r),
            _ => out.push(Ent { r, kids: vec![], seqend: None }),
        }
    }
    out
}

struct Writer<'a> {
    d: &'a Dxf,
    next: u64,
    layers: HashMap<String, u64>,
    ltypes: HashMap<String, u64>,
    styles: HashMap<String, u64>,
    dimstyles: HashMap<String, u64>,
    blocks: HashMap<String, u64>,
    /// Handles of the INSERTs of each block (by block record handle).
    inserts: HashMap<u64, Vec<u64>>,
    controls: HashMap<&'static str, u64>,
    model: u64,
    paper: u64,
    std_style: u64,
    std_dimstyle: u64,
    dict_root: u64,
    dict_group: u64,
    dict_layout: u64,
    dict_mline: u64,
    mline_std: u64,
    dict_plotsettings: u64,
    dict_plotstyle: u64,
    placeholder: u64,
    classes: Vec<usize>,
    objs: Vec<(u64, Vec<u8>)>,
    /// The first VIEWPORT of each block (a layout's last active viewport).
    viewports: HashMap<u64, u64>,
}

impl<'a> Writer<'a> {
    fn new(d: &'a Dxf) -> Result<Writer<'a>, String> {
        let seed = d.header.get("$HANDSEED").map_or(0, |r| hex(r.s(5)));
        let mut w = Writer {
            d,
            next: 0,
            layers: HashMap::new(),
            ltypes: HashMap::new(),
            styles: HashMap::new(),
            dimstyles: HashMap::new(),
            blocks: HashMap::new(),
            inserts: HashMap::new(),
            controls: HashMap::new(),
            model: 0,
            paper: 0,
            std_style: 0,
            std_dimstyle: 0,
            dict_root: 0,
            dict_group: 0,
            dict_layout: 0,
            dict_mline: 0,
            mline_std: 0,
            dict_plotsettings: 0,
            dict_plotstyle: 0,
            placeholder: 0,
            classes: vec![],
            objs: vec![],
            viewports: HashMap::new(),
        };
        // every handle in the DXF, so new objects get unused ones
        let mut top = seed;
        let mut scan = |r: &Rec| top = top.max(r.handle());
        for (t, e) in &d.tables {
            scan(t);
            e.iter().for_each(&mut scan);
        }
        for b in &d.blocks {
            scan(&b.begin);
            scan(&b.end);
            b.ents.iter().for_each(&mut scan);
        }
        d.entities.iter().for_each(&mut scan);
        d.objects.iter().for_each(&mut scan);
        w.next = top + 1;
        for (name, key) in [("BLOCK_RECORD", "BLOCK"), ("LAYER", "LAYER"), ("STYLE", "STYLE"), ("LTYPE", "LTYPE"), ("VIEW", "VIEW"), ("UCS", "UCS"), ("VPORT", "VPORT"), ("APPID", "APPID"), ("DIMSTYLE", "DIMSTYLE")] {
            let h = match d.table(name) {
                Some((t, _)) => t.handle(),
                None => w.alloc(),
            };
            w.controls.insert(key, h);
        }
        let vp = w.alloc();
        w.controls.insert("VP_ENT_HDR", vp);
        let names = |t: &str| -> HashMap<String, u64> { d.table(t).map(|(_, e)| e.iter().map(|r| (r.s(2).to_lowercase(), r.handle())).collect()).unwrap_or_default() };
        w.layers = names("LAYER");
        w.ltypes = names("LTYPE");
        w.styles = names("STYLE");
        w.dimstyles = names("DIMSTYLE");
        w.blocks = d.table("BLOCK_RECORD").map(|(_, e)| e.iter().map(|r| (r.s(2).to_string(), r.handle())).collect()).unwrap_or_default();
        w.model = *w.blocks.get("*Model_Space").ok_or("The drawing has no model space.")?;
        w.paper = *w.blocks.get("*Paper_Space").ok_or("The drawing has no paper space.")?;
        if !w.layers.contains_key("0") {
            return Err("The drawing has no layer 0.".into());
        }
        for lt in ["bylayer", "byblock", "continuous"] {
            if !w.ltypes.contains_key(lt) {
                return Err(format!("The drawing has no {lt} linetype."));
            }
        }
        w.std_style = w.styles.get("standard").copied().ok_or("The drawing has no Standard text style.")?;
        w.std_dimstyle = w.dimstyles.get("standard").copied().ok_or("The drawing has no Standard dimension style.")?;
        let root = d.objects.iter().find(|o| o.ty == "DICTIONARY" && o.owner() == 0).ok_or("The drawing has no named object dictionary.")?;
        w.dict_root = root.handle();
        let entry = |name: &str| root.g.windows(2).find(|p| p[0].0 == 3 && p[0].1 == name && p[1].0 == 350).map(|p| hex(&p[1].1));
        w.dict_group = entry("ACAD_GROUP").unwrap_or(0);
        w.dict_layout = entry("ACAD_LAYOUT").ok_or("The drawing has no layout dictionary.")?;
        w.dict_mline = w.alloc();
        w.mline_std = w.alloc();
        w.dict_plotsettings = w.alloc();
        w.dict_plotstyle = w.alloc();
        w.placeholder = w.alloc();
        // the INSERTs of each block, for the block records
        let mut all: Vec<&Rec> = d.entities.iter().collect();
        for b in &d.blocks {
            all.extend(b.ents.iter());
        }
        for r in all {
            if r.ty == "INSERT" {
                if let Some(&b) = w.blocks.get(r.s(2)) {
                    w.inserts.entry(b).or_default().push(r.handle());
                }
            }
        }
        Ok(w)
    }

    fn alloc(&mut self) -> u64 {
        self.next += 1;
        self.next - 1
    }

    fn class(&mut self, dxf: &str) -> u16 {
        let k = CLASSES.iter().position(|c| c.dxf == dxf).expect("known class");
        let i = match self.classes.iter().position(|&c| c == k) {
            Some(i) => i,
            None => {
                self.classes.push(k);
                self.classes.len() - 1
            }
        };
        500 + i as u16
    }

    fn put(&mut self, handle: u64, o: O) {
        self.objs.push((handle, o.finish()));
    }

    fn layer(&self, name: &str) -> u64 {
        self.layers.get(&name.to_lowercase()).or(self.layers.get("0")).copied().unwrap_or(0)
    }

    fn style(&self, name: &str) -> u64 {
        self.styles.get(&name.to_lowercase()).copied().unwrap_or(self.std_style)
    }

    fn dimstyle(&self, name: &str) -> u64 {
        self.dimstyles.get(&name.to_lowercase()).copied().unwrap_or(self.std_dimstyle)
    }

    fn nonent(&self, ty: u16, handle: u64, owner: u64, reactors: &[u64]) -> O {
        let mut o = O::new(ty, handle);
        o.h.h(4, owner);
        o.m.bl(reactors.len() as i32);
        for r in reactors {
            o.h.h(4, *r);
        }
        o.h.h(3, 0); // no extension dictionary
        o
    }

    fn ent(&self, ty: u16, r: &Rec, owner: u64, prev: u64, next: u64) -> O {
        let mut o = O::new(ty, r.handle());
        o.m.bit(false); // no proxy graphics
        let mode = if owner == self.model {
            2
        } else if owner == self.paper {
            1
        } else {
            0
        };
        o.m.bb(mode);
        if mode == 0 {
            o.h.h(4, owner);
        }
        o.m.bl(0);
        o.h.h(3, 0);
        o.m.bit(false); // previous and next entity given explicitly
        o.h.h(4, prev);
        o.h.h(4, next);
        o.m.bs(r.i(62, 256).clamp(0, 256) as i16);
        o.m.bd(r.fd(48, 1.0));
        o.h.h(5, self.layer(r.s(8)));
        match r.s(6).to_lowercase().as_str() {
            "" | "bylayer" => o.m.bb(0),
            "byblock" => o.m.bb(1),
            "continuous" => o.m.bb(2),
            lt => match self.ltypes.get(lt) {
                Some(&h) => {
                    o.m.bb(3);
                    o.h.h(5, h);
                }
                None => o.m.bb(0),
            },
        }
        o.m.bb(0); // plot style ByLayer
        o.m.bs(r.i(60, 0) as i16);
        o.m.rc(lw_index(r.i(370, -1)));
        o
    }

    fn write_all(&mut self) -> Result<(), String> {
        self.tables();
        let d = self.d;
        // blocks: the BLOCK_RECORD order, model space taking its entities from the ENTITIES section
        let recs: Vec<Rec> = d.table("BLOCK_RECORD").map(|(_, e)| e.clone()).unwrap_or_default();
        for rec in &recs {
            let name = rec.s(2);
            let Some(b) = d.blocks.iter().find(|b| b.begin.s(2) == name) else { continue };
            let list = if name == "*Model_Space" { &d.entities } else { &b.ents };
            self.block(rec, b, list)?;
        }
        self.objects();
        Ok(())
    }

    // ------------------------------------------------------------------------------------------------- tables

    fn control(&mut self, key: &'static str, ty: u16, entries: &[u64], tail: &[u64]) {
        let h = self.controls[key];
        let mut o = self.nonent(ty, h, 0, &[]);
        o.m.bl(entries.len() as i32);
        if key == "DIMSTYLE" {
            o.m.rc(0);
        }
        for e in entries {
            o.h.h(2, *e);
        }
        for t in tail {
            o.h.h(3, *t);
        }
        self.put(h, o);
    }

    fn entries(&self, table: &str) -> Vec<Rec> {
        self.d.table(table).map(|(_, e)| e.clone()).unwrap_or_default()
    }

    fn tables(&mut self) {
        // block records
        let recs = self.entries("BLOCK_RECORD");
        let others: Vec<u64> = recs.iter().filter(|r| !matches!(r.s(2), "*Model_Space" | "*Paper_Space")).map(|r| r.handle()).collect();
        let (ms, ps) = (self.model, self.paper);
        self.control("BLOCK", 48, &others, &[ms, ps]);
        // (the block records themselves are written with their blocks, which give their entity lists)
        let layers = self.entries("LAYER");
        self.control("LAYER", 50, &layers.iter().map(|r| r.handle()).collect::<Vec<_>>(), &[]);
        for r in &layers {
            self.layer_rec(r);
        }
        let styles = self.entries("STYLE");
        self.control("STYLE", 52, &styles.iter().map(|r| r.handle()).collect::<Vec<_>>(), &[]);
        for r in &styles {
            let mut o = self.nonent(53, r.handle(), self.controls["STYLE"], &[]);
            o.m.tv(r.s(2));
            o.xref();
            let flags = r.i(70, 0);
            o.m.bit(flags & 1 != 0);
            o.m.bit(flags & 4 != 0);
            o.m.bd(r.f(40));
            o.m.bd(r.fd(41, 1.0));
            o.m.bd(rad(r.f(50)));
            o.m.rc(r.i(71, 0) as u8);
            o.m.bd(r.fd(42, 2.5));
            o.m.tv(r.s(3));
            o.m.tv(r.s(4));
            o.h.h(5, 0);
            self.put(r.handle(), o);
        }
        let ltypes = self.entries("LTYPE");
        let (byblock, bylayer) = (self.ltypes["byblock"], self.ltypes["bylayer"]);
        let listed: Vec<u64> = ltypes.iter().map(|r| r.handle()).filter(|h| *h != byblock && *h != bylayer).collect();
        self.control("LTYPE", 56, &listed, &[byblock, bylayer]);
        for r in &ltypes {
            let mut o = self.nonent(57, r.handle(), self.controls["LTYPE"], &[]);
            o.m.tv(r.s(2));
            o.xref();
            o.m.tv(r.s(3));
            o.m.bd(r.f(40));
            o.m.rc(r.i(72, 65) as u8);
            let dashes: Vec<f64> = r.all(49).iter().map(|v| v.trim().parse().unwrap_or(0.0)).collect();
            o.m.rc(dashes.len().min(255) as u8);
            for dash in dashes.iter().take(255) {
                o.m.bd(*dash);
                o.m.bs(0); // shape number
                o.m.rd(0.0);
                o.m.rd(0.0);
                o.m.bd(1.0); // scale
                o.m.bd(0.0); // rotation
                o.m.bs(0); // shape flags
            }
            o.m.bytes(&[0u8; 256]); // text area
            o.h.h(5, 0);
            for _ in dashes.iter().take(255) {
                o.h.h(5, 0);
            }
            self.put(r.handle(), o);
        }
        let views = self.entries("VIEW");
        self.control("VIEW", 60, &views.iter().map(|r| r.handle()).collect::<Vec<_>>(), &[]);
        let ucss = self.entries("UCS");
        self.control("UCS", 62, &ucss.iter().map(|r| r.handle()).collect::<Vec<_>>(), &[]);
        let vports = self.entries("VPORT");
        self.control("VPORT", 64, &vports.iter().map(|r| r.handle()).collect::<Vec<_>>(), &[]);
        for r in &vports {
            self.vport(r);
        }
        let appids = self.entries("APPID");
        self.control("APPID", 66, &appids.iter().map(|r| r.handle()).collect::<Vec<_>>(), &[]);
        for r in &appids {
            let mut o = self.nonent(67, r.handle(), self.controls["APPID"], &[]);
            o.m.tv(r.s(2));
            o.xref();
            o.m.rc(0);
            o.h.h(5, 0);
            self.put(r.handle(), o);
        }
        let dimstyles = self.entries("DIMSTYLE");
        self.control("DIMSTYLE", 68, &dimstyles.iter().map(|r| r.handle()).collect::<Vec<_>>(), &[]);
        for r in &dimstyles {
            self.dimstyle_rec(r);
        }
        self.control("VP_ENT_HDR", 70, &[], &[]);
    }

    fn layer_rec(&mut self, r: &Rec) {
        let mut o = self.nonent(51, r.handle(), self.controls["LAYER"], &[]);
        o.m.tv(r.s(2));
        o.xref();
        let flags = r.i(70, 0);
        let color = r.i(62, 7);
        let mut v = (lw_index(r.i(370, -3)) as i16) << 5;
        if flags & 1 != 0 {
            v |= 1;
        }
        if color < 0 {
            v |= 2;
        }
        if flags & 2 != 0 {
            v |= 4;
        }
        if flags & 4 != 0 {
            v |= 8;
        }
        if r.i(290, 1) != 0 {
            v |= 16;
        }
        o.m.bs(v);
        o.m.bs(color.unsigned_abs().clamp(1, 255) as i16);
        o.h.h(5, 0); // xref block
        o.h.h(5, self.placeholder); // plot style "Normal"
        let lt = self.ltypes.get(&r.s(6).to_lowercase()).copied().unwrap_or(self.ltypes["continuous"]);
        o.h.h(5, lt);
        self.put(r.handle(), o);
    }

    fn vport(&mut self, r: &Rec) {
        let mut o = self.nonent(65, r.handle(), self.controls["VPORT"], &[]);
        o.m.tv(r.s(2));
        o.xref();
        let height = r.fd(40, 100.0);
        o.m.bd(height);
        o.m.bd(r.fd(41, 1.5) * height);
        o.m.rd2(r.p2(12));
        o.m.bd3(r.p(17));
        o.m.bd3([r.f(16), r.f(26), r.fd(36, 1.0)]);
        o.m.bd(rad(r.f(51)));
        o.m.bd(r.fd(42, 50.0));
        o.m.bd(r.f(43));
        o.m.bd(r.f(44));
        let mode = r.i(71, 0);
        o.m.bit(mode & 1 != 0);
        o.m.bit(mode & 2 != 0);
        o.m.bit(mode & 4 != 0);
        o.m.bit(mode & 16 != 0);
        o.m.rc(r.i(281, 0) as u8);
        o.m.rd2([r.f(10), r.f(20)]);
        o.m.rd2([r.fd(11, 1.0), r.fd(21, 1.0)]);
        o.m.bit(mode & 8 != 0);
        o.m.bs(r.i(72, 100) as i16);
        o.m.bit(true); // fast zoom
        let icon = r.i(74, 3);
        o.m.bit(icon & 1 != 0);
        o.m.bit(icon & 2 != 0);
        o.m.bit(r.i(76, 0) != 0);
        o.m.rd2([r.fd(15, 10.0), r.fd(25, 10.0)]);
        o.m.bit(r.i(75, 0) != 0);
        o.m.bit(r.i(77, 0) != 0);
        o.m.bs(r.i(78, 0) as i16);
        o.m.bd(rad(r.f(50)));
        o.m.rd2([r.f(13), r.f(23)]);
        o.m.rd2([r.fd(14, 10.0), r.fd(24, 10.0)]);
        o.m.bit(false);
        o.m.bit(true); // UCS per viewport
        o.m.bd3([0.0; 3]);
        o.m.bd3([1.0, 0.0, 0.0]);
        o.m.bd3([0.0, 1.0, 0.0]);
        o.m.bd(0.0);
        o.m.bs(0);
        o.h.h(5, 0); // xref block
        o.h.h(5, 0); // named UCS
        o.h.h(5, 0); // base UCS
        self.put(r.handle(), o);
    }

    fn dimstyle_rec(&mut self, r: &Rec) {
        let mut o = self.nonent(69, r.handle(), self.controls["DIMSTYLE"], &[]);
        o.m.tv(r.s(2));
        o.xref();
        o.m.tv(r.s(3)); // DIMPOST
        o.m.tv(r.s(4)); // DIMAPOST
        for (c, d) in [(40, 1.0), (41, 2.5), (42, 0.625), (43, 3.75), (44, 1.25), (45, 0.0), (46, 0.0), (47, 0.0), (48, 0.0)] {
            o.m.bd(r.fd(c, d));
        }
        for c in [71, 72, 73, 74, 75, 76] {
            o.m.bit(r.i(c, 0) != 0);
        }
        o.m.bs(r.i(77, 1) as i16); // DIMTAD
        o.m.bs(r.i(78, 8) as i16); // DIMZIN
        o.m.bs(r.i(79, 0) as i16); // DIMAZIN
        for (c, d) in [(140, 2.5), (141, 2.5), (142, 0.0), (143, 0.03937007874), (144, 1.0), (145, 0.0), (146, 1.0), (147, 0.625), (148, 0.0)] {
            o.m.bd(r.fd(c, d));
        }
        o.m.bit(r.i(170, 0) != 0); // DIMALT
        o.m.bs(r.i(171, 3) as i16); // DIMALTD
        for c in [172, 173, 174, 175] {
            o.m.bit(r.i(c, 0) != 0); // DIMTOFL DIMSAH DIMTIX DIMSOXD
        }
        for c in [176, 177, 178] {
            o.m.bs(r.i(c, 0) as i16); // DIMCLRD DIMCLRE DIMCLRT
        }
        for (c, d) in [(179, 0), (271, 2), (272, 2), (273, 2), (274, 2), (275, 0), (276, 0), (277, 2), (278, b'.' as i64), (279, 0), (280, 0)] {
            o.m.bs(r.i(c, d) as i16);
        }
        o.m.bit(r.i(281, 0) != 0); // DIMSD1
        o.m.bit(r.i(282, 0) != 0); // DIMSD2
        for (c, d) in [(283, 0), (284, 8), (285, 0), (286, 0)] {
            o.m.bs(r.i(c, d) as i16);
        }
        o.m.bit(r.i(288, 0) != 0); // DIMUPT
        o.m.bs(r.i(289, 3) as i16); // DIMATFIT
        o.m.bs(r.i(371, -2) as i16); // DIMLWD
        o.m.bs(r.i(372, -2) as i16); // DIMLWE
        o.m.bit(false);
        o.h.h(5, 0); // xref block
        o.h.h(5, self.std_style); // DIMTXSTY
        for _ in 0..4 {
            o.h.h(5, 0); // DIMLDRBLK DIMBLK DIMBLK1 DIMBLK2
        }
        self.put(r.handle(), o);
    }

    // ------------------------------------------------------------------------------------------------- blocks

    fn block(&mut self, rec: &Rec, b: &dxfin::Block, list: &[Rec]) -> Result<(), String> {
        let rh = rec.handle();
        let ents = group(list);
        let firsts: Vec<u64> = ents.iter().map(|e| e.r.handle()).collect();
        // block record
        let name = rec.s(2);
        let flags = b.begin.i(70, 0);
        let anon = flags & 1 != 0;
        let layout = rec.get(340).map_or(0, hex);
        let mut o = self.nonent(49, rh, self.controls["BLOCK"], &[]);
        let stored = if layout != 0 && name.starts_with("*Paper_Space") {
            "*Paper_Space".to_string()
        } else if anon {
            name.chars().take(2).collect()
        } else {
            name.to_string()
        };
        o.m.tv(&stored);
        o.xref();
        o.m.bit(anon);
        o.m.bit(list.iter().any(|e| e.ty == "ATTDEF"));
        o.m.bit(false); // xref
        o.m.bit(false); // overlaid xref
        o.m.bit(false); // loaded
        o.m.bd3(b.begin.p(10));
        o.m.tv(b.begin.s(1));
        let ins = self.inserts.get(&rh).cloned().unwrap_or_default();
        for _ in &ins {
            o.m.rc(1);
        }
        o.m.rc(0);
        o.m.tv(b.begin.s(4)); // description
        o.m.bl(0); // no preview
        o.h.h(5, 0);
        o.h.h(3, b.begin.handle());
        o.h.h(4, firsts.first().copied().unwrap_or(0));
        o.h.h(4, firsts.last().copied().unwrap_or(0));
        o.h.h(3, b.end.handle());
        for i in &ins {
            o.h.h(4, *i);
        }
        o.h.h(5, layout);
        self.put(rh, o);
        // BLOCK, entities, ENDBLK
        let mut o = self.ent(4, &b.begin, rh, 0, 0);
        o.m.tv(name);
        self.put(b.begin.handle(), o);
        if let Some(v) = ents.iter().find(|e| e.r.ty == "VIEWPORT") {
            self.viewports.insert(rh, v.r.handle());
        }
        for (k, e) in ents.iter().enumerate() {
            let prev = if k > 0 { firsts[k - 1] } else { 0 };
            let next = firsts.get(k + 1).copied().unwrap_or(0);
            self.entity(e, rh, prev, next)?;
        }
        let o = self.ent(5, &b.end, rh, 0, 0);
        self.put(b.end.handle(), o);
        Ok(())
    }

    // ------------------------------------------------------------------------------------------------- entities

    fn entity(&mut self, e: &Ent, owner: u64, prev: u64, next: u64) -> Result<(), String> {
        let r = e.r;
        let h = r.handle();
        let ext = [r.f(210), r.f(220), r.fd(230, 1.0)];
        let o = match r.ty.as_str() {
            "LINE" => {
                let mut o = self.ent(19, r, owner, prev, next);
                let (a, b) = (r.p(10), r.p(11));
                let flat = a[2] == 0.0 && b[2] == 0.0;
                o.m.bit(flat);
                o.m.rd(a[0]);
                o.m.dd(b[0], a[0]);
                o.m.rd(a[1]);
                o.m.dd(b[1], a[1]);
                if !flat {
                    o.m.rd(a[2]);
                    o.m.dd(b[2], a[2]);
                }
                o.m.bt(r.f(39));
                o.m.be(ext);
                o
            }
            "CIRCLE" | "ARC" => {
                let mut o = self.ent(if r.ty == "ARC" { 17 } else { 18 }, r, owner, prev, next);
                o.m.bd3(r.p(10));
                o.m.bd(r.f(40));
                o.m.bt(r.f(39));
                o.m.be(ext);
                if r.ty == "ARC" {
                    o.m.bd(rad(r.f(50)));
                    o.m.bd(rad(r.f(51)));
                }
                o
            }
            "POINT" => {
                let mut o = self.ent(27, r, owner, prev, next);
                o.m.bd3(r.p(10));
                o.m.bt(r.f(39));
                o.m.be(ext);
                o.m.bd(rad(r.f(50)));
                o
            }
            "ELLIPSE" => {
                let mut o = self.ent(35, r, owner, prev, next);
                o.m.bd3(r.p(10));
                o.m.bd3(r.p(11));
                o.m.bd3(ext);
                o.m.bd(r.fd(40, 1.0));
                o.m.bd(r.f(41));
                o.m.bd(r.fd(42, std::f64::consts::TAU));
                o
            }
            "RAY" | "XLINE" => {
                let mut o = self.ent(if r.ty == "RAY" { 40 } else { 41 }, r, owner, prev, next);
                o.m.bd3(r.p(10));
                o.m.bd3(r.p(11));
                o
            }
            "SOLID" => {
                let mut o = self.ent(31, r, owner, prev, next);
                o.m.bt(r.f(39));
                o.m.bd(r.f(30));
                for c in [10, 11, 12, 13] {
                    o.m.rd2(r.p2(c));
                }
                o.m.be(ext);
                o
            }
            "TEXT" | "ATTRIB" | "ATTDEF" => {
                let ty = match r.ty.as_str() {
                    "TEXT" => 1,
                    "ATTRIB" => 2,
                    _ => 3,
                };
                let mut o = self.ent(ty, r, owner, prev, next);
                self.text_data(&mut o, r, if ty == 1 { 73 } else { 74 });
                if ty != 1 {
                    o.m.tv(r.s(2));
                    o.m.bs(0); // field length
                    o.m.rc(r.i(70, 0) as u8);
                }
                if ty == 3 {
                    o.m.tv(r.s(3));
                }
                o
            }
            "MTEXT" => {
                let mut o = self.ent(44, r, owner, prev, next);
                o.m.bd3(r.p(10));
                o.m.bd3(ext);
                o.m.bd3(if r.get(11).is_some() { r.p(11) } else { [1.0, 0.0, 0.0] });
                o.m.bd(r.f(41));
                o.m.bd(r.fd(40, 2.5));
                o.m.bs(r.i(71, 1) as i16);
                o.m.bs(r.i(72, 1) as i16);
                o.m.bd(0.0); // extents height
                o.m.bd(0.0); // extents width
                let mut text: String = r.all(3).concat();
                text.push_str(r.s(1));
                o.m.tv(&text);
                o.h.h(5, self.style(r.s(7)));
                o.m.bs(r.i(73, 1) as i16);
                o.m.bd(r.fd(44, 1.0));
                o.m.bit(false);
                o
            }
            "INSERT" => {
                let mut o = self.ent(7, r, owner, prev, next);
                o.m.bd3(r.p(10));
                let (sx, sy, sz) = (r.fd(41, 1.0), r.fd(42, 1.0), r.fd(43, 1.0));
                if sx == 1.0 && sy == 1.0 && sz == 1.0 {
                    o.m.bb(3);
                } else if sx == sy && sx == sz {
                    o.m.bb(2);
                    o.m.rd(sx);
                } else if sx == 1.0 {
                    o.m.bb(1);
                    o.m.dd(sy, 1.0);
                    o.m.dd(sz, 1.0);
                } else {
                    o.m.bb(0);
                    o.m.rd(sx);
                    o.m.dd(sy, sx);
                    o.m.dd(sz, sx);
                }
                o.m.bd(rad(r.f(50)));
                o.m.bd3(ext);
                let atts = !e.kids.is_empty() && e.seqend.is_some();
                o.m.bit(atts);
                let block = self.blocks.get(r.s(2)).copied().ok_or_else(|| format!("INSERT of a missing block {}", r.s(2)))?;
                o.h.h(5, block);
                if atts {
                    o.h.h(4, e.kids[0].handle());
                    o.h.h(4, e.kids[e.kids.len() - 1].handle());
                    o.h.h(3, e.seqend.map_or(0, |s| s.handle()));
                }
                o
            }
            "LWPOLYLINE" => {
                let mut o = self.ent(77, r, owner, prev, next);
                self.lwpolyline(&mut o, r, ext);
                o
            }
            "SPLINE" => {
                let mut o = self.ent(36, r, owner, prev, next);
                let flags = r.i(70, 0);
                let knots: Vec<f64> = r.all(40).iter().map(|v| v.trim().parse().unwrap_or(0.0)).collect();
                let ctrl = points(r, 10);
                o.m.bl(1); // control points (not fit points)
                o.m.bl(r.i(71, 3) as i32);
                o.m.bit(flags & 4 != 0);
                o.m.bit(flags & 1 != 0);
                o.m.bit(flags & 2 != 0);
                o.m.bd(r.fd(42, 1e-10));
                o.m.bd(r.fd(43, 1e-10));
                o.m.bl(knots.len() as i32);
                o.m.bl(ctrl.len() as i32);
                o.m.bit(false);
                for k in knots {
                    o.m.bd(k);
                }
                for p in ctrl {
                    o.m.bd3(p);
                }
                o
            }
            "HATCH" => {
                let mut o = self.ent(78, r, owner, prev, next);
                self.hatch(&mut o, r, ext);
                o
            }
            "LEADER" => {
                let mut o = self.ent(45, r, owner, prev, next);
                let pts = points(r, 10);
                o.m.bit(false);
                o.m.bs(r.i(73, 3) as i16);
                o.m.bs(r.i(72, 0) as i16);
                o.m.bl(pts.len() as i32);
                for p in &pts {
                    o.m.bd3(*p);
                }
                o.m.bd3(pts.first().copied().unwrap_or([0.0; 3]));
                o.m.bd3(ext);
                o.m.bd3([r.fd(211, 1.0), r.f(221), r.f(231)]);
                o.m.bd3([0.0; 3]); // offset to the block insertion point
                o.m.bd3([0.0; 3]); // offset to the annotation
                o.m.bd(r.f(40));
                o.m.bd(r.f(41));
                o.m.bit(r.i(74, 0) != 0);
                o.m.bit(r.i(71, 1) != 0);
                o.m.bs(0);
                o.m.bit(false);
                o.m.bit(false);
                o.h.h(5, 0); // annotation
                o.h.h(5, self.dimstyle(r.s(3)));
                o
            }
            "DIMENSION" => self.dimension(r, owner, prev, next, ext)?,
            "VIEWPORT" => {
                let mut o = self.ent(34, r, owner, prev, next);
                o.m.bd3(r.p(10));
                o.m.bd(r.f(40));
                o.m.bd(r.f(41));
                o.m.bd3(r.p(17));
                o.m.bd3([r.f(16), r.f(26), r.fd(36, 1.0)]);
                o.m.bd(rad(r.f(51)));
                o.m.bd(r.fd(45, 1.0));
                o.m.bd(r.fd(42, 50.0));
                o.m.bd(r.f(43));
                o.m.bd(r.f(44));
                o.m.bd(rad(r.f(50)));
                o.m.rd2(r.p2(12));
                o.m.rd2(r.p2(13));
                o.m.rd2([r.fd(14, 10.0), r.fd(24, 10.0)]);
                o.m.rd2([r.fd(15, 10.0), r.fd(25, 10.0)]);
                o.m.bs(r.i(72, 100) as i16);
                let frozen: Vec<u64> = r.all(331).iter().map(|v| hex(v)).collect();
                o.m.bl(frozen.len() as i32);
                let off = if r.i(68, 1) == 0 { 0x20000 } else { 0 };
                o.m.bl((r.i(90, 0) as i32) | off);
                o.m.tv(r.s(1));
                o.m.rc(r.i(281, 0) as u8);
                o.m.bit(r.i(74, 0) != 0);
                o.m.bit(r.i(71, 0) != 0);
                o.m.bd3(r.p(110));
                o.m.bd3([r.fd(111, 1.0), r.f(121), r.f(131)]);
                o.m.bd3([r.f(112), r.fd(122, 1.0), r.f(132)]);
                o.m.bd(r.f(146));
                o.m.bs(r.i(79, 0) as i16);
                for f in frozen {
                    o.h.h(5, f);
                }
                o.h.h(5, r.get(340).map_or(0, hex)); // clip boundary
                o.h.h(5, 0); // viewport entity header
                o.h.h(5, 0); // named UCS
                o.h.h(5, 0); // base UCS
                o
            }
            "IMAGE" => {
                let ty = self.class("IMAGE");
                let mut o = self.ent(ty, r, owner, prev, next);
                o.m.bl(r.i(90, 0) as i32);
                o.m.bd3(r.p(10));
                o.m.bd3(r.p(11));
                o.m.bd3(r.p(12));
                o.m.rd2(r.p2(13));
                o.m.bs(r.i(70, 3) as i16);
                o.m.bit(r.i(280, 0) != 0);
                o.m.rc(r.i(281, 50) as u8);
                o.m.rc(r.i(282, 50) as u8);
                o.m.rc(r.i(283, 0) as u8);
                let clip = points2(r, 14);
                let kind = r.i(71, 1);
                o.m.bs(kind as i16);
                if kind == 1 {
                    for k in 0..2 {
                        o.m.rd2(clip.get(k).copied().unwrap_or([0.0; 2]));
                    }
                } else {
                    o.m.bl(clip.len() as i32);
                    for p in clip {
                        o.m.rd2(p);
                    }
                }
                o.h.h(5, r.get(340).map_or(0, hex));
                o.h.h(3, r.get(360).map_or(0, hex));
                o
            }
            other => return Err(format!("CadForge cannot write {other} entities to DWG yet")),
        };
        self.put(h, o);
        // attributes, linked to each other, then their SEQEND
        let kids: Vec<u64> = e.kids.iter().map(|k| k.handle()).collect();
        for (i, k) in e.kids.iter().enumerate() {
            let prev = if i > 0 { kids[i - 1] } else { 0 };
            let next = kids.get(i + 1).copied().unwrap_or(0);
            self.entity(&Ent { r: k, kids: vec![], seqend: None }, h, prev, next)?;
        }
        if let Some(s) = e.seqend {
            let o = self.ent(6, s, h, 0, 0);
            self.put(s.handle(), o);
        }
        Ok(())
    }

    /// TEXT, ATTRIB and ATTDEF data (`vcode`: the DXF group of the vertical alignment).
    fn text_data(&self, o: &mut O, r: &Rec, vcode: i32) {
        let ins = r.p(10);
        let al = if r.get(11).is_some() { r.p(11) } else { [0.0; 3] };
        let (obl, rot, wf) = (rad(r.f(51)), rad(r.f(50)), r.fd(41, 1.0));
        let (gen, ha, va) = (r.i(71, 0), r.i(72, 0), r.i(vcode, 0));
        let mut fl = 0u8;
        for (bit, zero) in [(1, ins[2] == 0.0), (2, al == [0.0; 3]), (4, obl == 0.0), (8, rot == 0.0), (16, wf == 1.0), (32, gen == 0), (64, ha == 0), (128, va == 0)] {
            if zero {
                fl |= bit;
            }
        }
        o.m.rc(fl);
        if fl & 1 == 0 {
            o.m.rd(ins[2]);
        }
        o.m.rd(ins[0]);
        o.m.rd(ins[1]);
        if fl & 2 == 0 {
            o.m.dd(al[0], ins[0]);
            o.m.dd(al[1], ins[1]);
        }
        o.m.be([r.f(210), r.f(220), r.fd(230, 1.0)]);
        o.m.bt(r.f(39));
        if fl & 4 == 0 {
            o.m.rd(obl);
        }
        if fl & 8 == 0 {
            o.m.rd(rot);
        }
        o.m.rd(r.fd(40, 2.5));
        if fl & 16 == 0 {
            o.m.rd(wf);
        }
        o.m.tv(r.s(1));
        if fl & 32 == 0 {
            o.m.bs(gen as i16);
        }
        if fl & 64 == 0 {
            o.m.bs(ha as i16);
        }
        if fl & 128 == 0 {
            o.m.bs(va as i16);
        }
        o.h.h(5, self.style(r.s(7)));
    }

    fn lwpolyline(&self, o: &mut O, r: &Rec, ext: [f64; 3]) {
        // vertices: 10/20, then their own 40/41 widths and 42 bulge
        let mut v: Vec<[f64; 5]> = vec![];
        let mut x = None;
        for (c, s) in &r.g {
            let f: f64 = s.trim().parse().unwrap_or(0.0);
            match *c {
                10 => x = Some(f),
                20 => {
                    if let Some(x) = x.take() {
                        v.push([x, f, 0.0, 0.0, 0.0]);
                    }
                }
                40..=42 => {
                    if let Some(last) = v.last_mut() {
                        last[match *c {
                            40 => 2,
                            41 => 3,
                            _ => 4,
                        }] = f;
                    }
                }
                _ => {}
            }
        }
        let bulges = v.iter().any(|p| p[4] != 0.0);
        let widths = v.iter().any(|p| p[2] != 0.0 || p[3] != 0.0);
        let (cw, elev, thick) = (r.f(43), r.f(38), r.f(39));
        let mut flags = 0i16;
        if r.i(70, 0) & 1 != 0 {
            flags |= 0x200;
        }
        if r.i(70, 0) & 128 != 0 {
            flags |= 0x100;
        }
        if cw != 0.0 {
            flags |= 4;
        }
        if elev != 0.0 {
            flags |= 8;
        }
        if thick != 0.0 {
            flags |= 2;
        }
        if ext != [0.0, 0.0, 1.0] {
            flags |= 1;
        }
        if bulges {
            flags |= 0x10;
        }
        if widths {
            flags |= 0x20;
        }
        o.m.bs(flags);
        if cw != 0.0 {
            o.m.bd(cw);
        }
        if elev != 0.0 {
            o.m.bd(elev);
        }
        if thick != 0.0 {
            o.m.bd(thick);
        }
        if ext != [0.0, 0.0, 1.0] {
            o.m.bd3(ext);
        }
        o.m.bl(v.len() as i32);
        if bulges {
            o.m.bl(v.len() as i32);
        }
        if widths {
            o.m.bl(v.len() as i32);
        }
        for (k, p) in v.iter().enumerate() {
            if k == 0 {
                o.m.rd2([p[0], p[1]]);
            } else {
                o.m.dd(p[0], v[k - 1][0]);
                o.m.dd(p[1], v[k - 1][1]);
            }
        }
        if bulges {
            for p in &v {
                o.m.bd(p[4]);
            }
        }
        if widths {
            for p in &v {
                o.m.bd(p[2]);
                o.m.bd(p[3]);
            }
        }
    }

    fn hatch(&self, o: &mut O, r: &Rec, ext: [f64; 3]) {
        let solid = r.i(70, 0) != 0;
        o.m.bd(r.f(30));
        o.m.bd3(ext);
        o.m.tv(r.s(2));
        o.m.bit(solid);
        o.m.bit(r.i(71, 0) != 0);
        // boundary paths (CadForge writes polyline paths): 92 flags, 72 bulges, 73 closed, 93 count, 10/20/42
        struct Path {
            flags: i32,
            bulges: bool,
            closed: bool,
            v: Vec<[f64; 3]>,
        }
        let mut paths: Vec<Path> = vec![];
        // pattern lines: angle, base point, offset, dashes
        type Family = (f64, [f64; 2], [f64; 2], Vec<f64>);
        let mut defs: Vec<Family> = vec![];
        let mut seeds: Vec<[f64; 2]> = vec![];
        let mut stage = 0; // 0 before the paths, 1 in a path, 2 pattern, 3 seeds
        let mut x = None;
        for (c, s) in &r.g {
            let f: f64 = s.trim().parse().unwrap_or(0.0);
            let n = f as i32;
            match (*c, stage) {
                (92, _) => {
                    stage = 1;
                    paths.push(Path { flags: n, bulges: false, closed: true, v: vec![] });
                }
                (72, 1) => paths.last_mut().expect("path").bulges = n != 0,
                (73, 1) => paths.last_mut().expect("path").closed = n != 0,
                (10, 1) | (10, 3) => x = Some(f),
                (20, 1) => {
                    if let Some(x) = x.take() {
                        paths.last_mut().expect("path").v.push([x, f, 0.0]);
                    }
                }
                (42, 1) => {
                    if let Some(p) = paths.last_mut().and_then(|p| p.v.last_mut()) {
                        p[2] = f;
                    }
                }
                (75, _) => stage = 2,
                (53, 2) => defs.push((f, [0.0; 2], [0.0; 2], vec![])),
                (43, 2) => defs.last_mut().map_or((), |d| d.1[0] = f),
                (44, 2) => defs.last_mut().map_or((), |d| d.1[1] = f),
                (45, 2) => defs.last_mut().map_or((), |d| d.2[0] = f),
                (46, 2) => defs.last_mut().map_or((), |d| d.2[1] = f),
                (49, 2) => defs.last_mut().map_or((), |d| d.3.push(f)),
                (98, _) => stage = 3,
                (20, 3) => {
                    if let Some(x) = x.take() {
                        seeds.push([x, f]);
                    }
                }
                _ => {}
            }
        }
        o.m.bl(paths.len() as i32);
        for p in &paths {
            o.m.bl(p.flags | 2);
            o.m.bit(p.bulges);
            o.m.bit(p.closed);
            o.m.bl(p.v.len() as i32);
            for v in &p.v {
                o.m.rd2([v[0], v[1]]);
                if p.bulges {
                    o.m.bd(v[2]);
                }
            }
            o.m.bl(0); // no associated boundary objects
        }
        o.m.bs(r.i(75, 0) as i16);
        o.m.bs(r.i(76, 1) as i16);
        if !solid {
            o.m.bd(rad(r.f(52)));
            o.m.bd(r.fd(41, 1.0));
            o.m.bit(r.i(77, 0) != 0);
            o.m.bs(defs.len() as i16);
            for (a, base, off, dash) in &defs {
                o.m.bd(rad(*a));
                o.m.bd2(*base);
                o.m.bd2(*off);
                o.m.bs(dash.len() as i16);
                for d in dash {
                    o.m.bd(*d);
                }
            }
        }
        o.m.bl(seeds.len() as i32);
        for s in seeds {
            o.m.rd2(s);
        }
    }

    fn dimension(&self, r: &Rec, owner: u64, prev: u64, next: u64, ext: [f64; 3]) -> Result<O, String> {
        let kind = r.i(70, 0);
        let ty = match kind & 7 {
            0 => 21,
            1 => 22,
            2 => 24,
            3 => 26,
            4 => 25,
            5 => 23,
            _ => 20,
        };
        let mut o = self.ent(ty, r, owner, prev, next);
        o.m.bd3(ext);
        let mid = r.p(11);
        o.m.rd2([mid[0], mid[1]]);
        o.m.bd(mid[2]);
        o.m.rc((kind & 128 == 0) as u8 | if kind & 32 != 0 { 2 } else { 0 });
        o.m.tv(r.s(1));
        o.m.bd(rad(r.f(53)));
        o.m.bd(rad(r.f(51)));
        o.m.bd3([1.0, 1.0, 1.0]);
        o.m.bd(0.0);
        o.m.bs(r.i(71, 5) as i16);
        o.m.bs(r.i(72, 1) as i16);
        o.m.bd(r.fd(41, 1.0));
        o.m.bd(r.f(42));
        o.m.rd2(r.p2(12));
        match ty {
            21 | 22 => {
                o.m.bd3(r.p(13));
                o.m.bd3(r.p(14));
                o.m.bd3(r.p(10));
                o.m.bd(rad(r.f(52)));
                if ty == 21 {
                    o.m.bd(rad(r.f(50)));
                }
            }
            25 => {
                o.m.bd3(r.p(10));
                o.m.bd3(r.p(15));
                o.m.bd(r.f(40));
            }
            26 => {
                o.m.bd3(r.p(15));
                o.m.bd3(r.p(10));
                o.m.bd(r.f(40));
            }
            23 => {
                for c in [10, 13, 14, 15] {
                    o.m.bd3(r.p(c));
                }
            }
            24 => {
                o.m.rd2(r.p2(16));
                for c in [13, 14, 15, 10] {
                    o.m.bd3(r.p(c));
                }
            }
            _ => {
                for c in [10, 13, 14] {
                    o.m.bd3(r.p(c));
                }
                o.m.rc((kind & 64 != 0) as u8);
            }
        }
        o.h.h(5, self.dimstyle(r.s(3)));
        let block = self.blocks.get(r.s(2)).copied().ok_or_else(|| format!("DIMENSION with a missing block {}", r.s(2)))?;
        o.h.h(5, block);
        Ok(o)
    }

    // ------------------------------------------------------------------------------------------------- objects

    fn dictionary(&self, o: &mut O, entries: &[(String, u64)], cloning: i16) {
        o.m.bl(entries.len() as i32);
        o.m.bs(cloning);
        o.m.rc(0);
        for (name, h) in entries {
            o.m.tv(name);
            o.h.h(2, *h);
        }
    }

    fn objects(&mut self) {
        let d = self.d;
        let mut root_done = false;
        for r in &d.objects {
            let h = r.handle();
            match r.ty.as_str() {
                "DICTIONARY" => {
                    let mut entries: Vec<(String, u64)> = r.g.windows(2).filter(|p| p[0].0 == 3 && matches!(p[1].0, 350 | 360)).map(|p| (p[0].1.clone(), hex(&p[1].1))).collect();
                    if h == self.dict_root {
                        entries.push(("ACAD_MLINESTYLE".into(), self.dict_mline));
                        entries.push(("ACAD_PLOTSETTINGS".into(), self.dict_plotsettings));
                        entries.push(("ACAD_PLOTSTYLENAME".into(), self.dict_plotstyle));
                        entries.sort_by_key(|e| e.0.to_uppercase());
                        root_done = true;
                    }
                    let mut o = self.nonent(42, h, r.owner(), &r.reactors());
                    self.dictionary(&mut o, &entries, r.i(281, 1) as i16);
                    self.put(h, o);
                }
                "LAYOUT" => {
                    let ty = self.class("LAYOUT");
                    let mut o = self.nonent(ty, h, r.owner(), &r.reactors());
                    // the plot settings part and the layout part each have their own group 1, 70 and 330
                    let at = r.g.iter().position(|(c, v)| *c == 100 && v == "AcDbLayout").unwrap_or(r.g.len());
                    let p = Rec { ty: String::new(), g: r.g[..at].to_vec() };
                    let l = Rec { ty: String::new(), g: r.g[at..].to_vec() };
                    o.m.tv(p.s(1));
                    o.m.tv(p.s(2));
                    o.m.bs(p.i(70, 688) as i16);
                    for c in [40, 41, 42, 43, 44, 45] {
                        o.m.bd(p.f(c));
                    }
                    o.m.tv(p.s(4));
                    o.m.bd(p.f(46));
                    o.m.bd(p.f(47));
                    o.m.bs(p.i(72, 1) as i16);
                    o.m.bs(p.i(73, 0) as i16);
                    o.m.bs(p.i(74, 5) as i16);
                    for c in [48, 49, 140, 141] {
                        o.m.bd(p.f(c));
                    }
                    o.m.tv(p.s(6));
                    o.m.bd(p.fd(142, 1.0));
                    o.m.bd(p.fd(143, 1.0));
                    o.m.tv(p.s(7));
                    o.m.bs(p.i(75, 16) as i16);
                    o.m.bd(p.fd(147, 1.0));
                    o.m.bd2([p.f(148), p.f(149)]);
                    o.m.tv(l.s(1));
                    o.m.bl(l.i(71, 0) as i32);
                    o.m.bs(l.i(70, 1) as i16);
                    o.m.bd3(l.p(13));
                    o.m.rd2(l.p2(10));
                    o.m.rd2(l.p2(11));
                    o.m.bd3(l.p(12));
                    o.m.bd3([l.fd(16, 1.0), l.f(26), l.f(36)]);
                    o.m.bd3([l.f(17), l.fd(27, 1.0), l.f(37)]);
                    o.m.bd(l.f(146));
                    o.m.bs(l.i(76, 0) as i16);
                    o.m.bd3(l.p(14));
                    o.m.bd3(l.p(15));
                    let block = l.get(330).map_or(0, hex);
                    o.h.h(4, block);
                    o.h.h(4, l.get(331).map_or_else(|| self.viewports.get(&block).copied().unwrap_or(0), hex));
                    o.h.h(5, 0); // base UCS
                    o.h.h(5, 0); // named UCS
                    self.put(h, o);
                }
                "IMAGEDEF" => {
                    let ty = self.class("IMAGEDEF");
                    let mut o = self.nonent(ty, h, r.owner(), &r.reactors());
                    o.m.bl(r.i(90, 0) as i32);
                    o.m.rd2(r.p2(10));
                    o.m.tv(r.s(1));
                    o.m.bit(r.i(280, 1) != 0);
                    o.m.rc(r.i(281, 0) as u8);
                    o.m.rd2([r.fd(11, 1.0), r.fd(21, 1.0)]);
                    self.put(h, o);
                }
                "IMAGEDEF_REACTOR" => {
                    let ty = self.class("IMAGEDEF_REACTOR");
                    let mut o = self.nonent(ty, h, r.owner(), &r.reactors());
                    o.m.bl(r.i(90, 2) as i32);
                    self.put(h, o);
                }
                _ => {}
            }
        }
        if !root_done {
            return;
        }
        // what a DWG needs and the DXF leaves out: the multiline style and the plot style dictionaries
        let (root, ml, mls, ps, pst, ph) = (self.dict_root, self.dict_mline, self.mline_std, self.dict_plotsettings, self.dict_plotstyle, self.placeholder);
        let mut o = self.nonent(42, ml, root, &[root]);
        self.dictionary(&mut o, &[("Standard".into(), mls)], 1);
        self.put(ml, o);
        let mut o = self.nonent(73, mls, ml, &[ml]);
        o.m.tv("Standard");
        o.m.tv("");
        o.m.bs(0);
        o.m.bs(256);
        o.m.bd(std::f64::consts::FRAC_PI_2);
        o.m.bd(std::f64::consts::FRAC_PI_2);
        o.m.rc(2);
        for off in [0.5, -0.5] {
            o.m.bd(off);
            o.m.bs(256);
            o.m.bs(32767); // ByLayer linetype
        }
        self.put(mls, o);
        let mut o = self.nonent(42, ps, root, &[root]);
        self.dictionary(&mut o, &[], 1);
        self.put(ps, o);
        let ty = self.class("ACDBDICTIONARYWDFLT");
        let mut o = self.nonent(ty, pst, root, &[root]);
        self.dictionary(&mut o, &[("Normal".into(), ph)], 1);
        o.h.h(5, ph);
        self.put(pst, o);
        let ty = self.class("ACDBPLACEHOLDER");
        let o = self.nonent(ty, ph, pst, &[pst]);
        self.put(ph, o);
    }

    // ------------------------------------------------------------------------------------------------- the file

    fn classes_section(&self) -> Vec<u8> {
        let mut m = BitW::new();
        for (i, &k) in self.classes.iter().enumerate() {
            let c = &CLASSES[k];
            m.bs(500 + i as i16);
            m.bs(c.proxy);
            m.tv(c.app);
            m.tv(c.cpp);
            m.tv(c.dxf);
            m.bit(false);
            m.bs(if c.entity { 0x1F2 } else { 0x1F3 });
        }
        m.align();
        let mut out = vec![0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF, 0xB6, 0x8A];
        let mut body = (m.buf.len() as i32).to_le_bytes().to_vec();
        body.extend_from_slice(&m.buf);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc16(0xC0C1, &body).to_le_bytes());
        out.extend_from_slice(&[0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30, 0x49, 0x75]);
        out
    }

    /// The object map: (handle, offset) pairs as deltas, in pages of at most 2032 bytes with a big-endian size and CRC.
    fn handles_section(map: &[(u64, usize)]) -> Vec<u8> {
        fn mc_unsigned(mut v: u64, out: &mut Vec<u8>) {
            while v >= 0x80 {
                out.push((v & 0x7F) as u8 | 0x80);
                v >>= 7;
            }
            out.push(v as u8);
        }
        fn mc_signed(v: i64, out: &mut Vec<u8>) {
            let (neg, mut a) = (v < 0, v.unsigned_abs());
            while a >= 0x40 {
                out.push((a & 0x7F) as u8 | 0x80);
                a >>= 7;
            }
            out.push(a as u8 | if neg { 0x40 } else { 0 });
        }
        let mut out = vec![];
        let mut page: Vec<u8> = vec![];
        let (mut last_h, mut last_off) = (0u64, 0i64);
        let flush = |page: &mut Vec<u8>, out: &mut Vec<u8>| {
            let size = (page.len() + 2) as u16;
            let mut p = size.to_be_bytes().to_vec();
            p.extend_from_slice(page);
            let crc = crc16(0xC0C1, &p);
            out.extend_from_slice(&p);
            out.extend_from_slice(&crc.to_be_bytes());
            page.clear();
        };
        for &(h, off) in map {
            let mut rec = vec![];
            mc_unsigned(h - last_h, &mut rec);
            mc_signed(off as i64 - last_off, &mut rec);
            if page.len() + rec.len() + 2 > 2032 {
                // a new page starts its deltas from zero
                flush(&mut page, &mut out);
                rec.clear();
                mc_unsigned(h, &mut rec);
                mc_signed(off as i64, &mut rec);
            }
            page.extend_from_slice(&rec);
            last_h = h;
            last_off = off as i64;
        }
        flush(&mut page, &mut out);
        flush(&mut page, &mut out); // an empty page ends the map
        out
    }

    fn file(&mut self) -> Vec<u8> {
        let julian = julian_now();
        let c = &self.controls;
        let refs = header::Refs {
            handseed: self.next,
            clayer: self.layer(self.d.header.get("$CLAYER").map_or("0", |r| r.s(8))),
            textstyle: self.std_style,
            celtype: self.ltypes["bylayer"],
            dimstyle: self.std_dimstyle,
            cmlstyle: self.mline_std,
            dimtxsty: self.std_style,
            controls: [c["BLOCK"], c["LAYER"], c["STYLE"], c["LTYPE"], c["VIEW"], c["UCS"], c["VPORT"], c["APPID"], c["DIMSTYLE"], c["VP_ENT_HDR"]],
            dict_group: self.dict_group,
            dict_mline: self.dict_mline,
            dict_root: self.dict_root,
            dict_layout: self.dict_layout,
            dict_plotsettings: self.dict_plotsettings,
            dict_plotstyle: self.dict_plotstyle,
            paper: self.paper,
            model: self.model,
            bylayer: self.ltypes["bylayer"],
            byblock: self.ltypes["byblock"],
            continuous: self.ltypes["continuous"],
        };
        let header = header::write(&self.d.header, &refs, julian);
        let classes = self.classes_section();
        let aux = aux_header(julian, self.next);
        let preview = preview();
        // file order: file header, aux header, preview, header, classes, objects, object map, free space, template
        let head_len = 0x61;
        let objects_at = head_len + aux.len() + preview.len() + header.len() + classes.len();
        let mut objects = vec![];
        let mut map = vec![];
        self.objs.sort_by_key(|o| o.0);
        for (h, bytes) in &self.objs {
            map.push((*h, objects_at + objects.len()));
            objects.extend_from_slice(bytes);
        }
        let handles = Self::handles_section(&map);
        let mut free = vec![];
        free.extend_from_slice(&0i32.to_le_bytes());
        free.extend_from_slice(&(map.len() as u32).to_le_bytes());
        free.extend_from_slice(&julian.0.to_le_bytes());
        free.extend_from_slice(&julian.1.to_le_bytes());
        free.extend_from_slice(&(objects_at as u32).to_le_bytes());
        free.push(4);
        for v in [0x32u32, 0, 0x64, 0, 0x200, 0, 0xFFFF_FFFF, 0] {
            free.extend_from_slice(&v.to_le_bytes());
        }
        let metric = self.d.header.get("$MEASUREMENT").map_or(1, |r| r.i(70, 1));
        let template = [0u8, 0, metric as u8, 0];
        let aux_at = head_len;
        let preview_at = aux_at + aux.len();
        let header_at = preview_at + preview.len();
        let classes_at = header_at + header.len();
        let handles_at = objects_at + objects.len();
        let free_at = handles_at + handles.len();
        let template_at = free_at + free.len();
        let mut f = b"AC1015".to_vec();
        f.extend_from_slice(&[0, 0, 0, 0, 0, 15, 1]);
        f.extend_from_slice(&(preview_at as u32).to_le_bytes());
        f.extend_from_slice(&[0x1B, 0x19]);
        f.extend_from_slice(&30u16.to_le_bytes()); // ANSI_1252
        f.extend_from_slice(&6u32.to_le_bytes());
        for (n, at, len) in [(0u8, header_at, header.len()), (1, classes_at, classes.len()), (2, handles_at, handles.len()), (3, free_at, free.len()), (4, template_at, template.len()), (5, aux_at, aux.len())] {
            f.push(n);
            f.extend_from_slice(&(at as u32).to_le_bytes());
            f.extend_from_slice(&(len as u32).to_le_bytes());
        }
        let crc = crc16(0xC0C1, &f);
        f.extend_from_slice(&crc.to_le_bytes());
        f.extend_from_slice(&[0x95, 0xA0, 0x4E, 0x28, 0x99, 0x82, 0x1A, 0xE5, 0x5E, 0x41, 0xE0, 0x5F, 0x9D, 0x3A, 0x4D, 0x00]);
        debug_assert_eq!(f.len(), head_len);
        for part in [&aux[..], &preview, &header, &classes, &objects, &handles, &free, &template] {
            f.extend_from_slice(part);
        }
        f
    }
}

/// Points in groups c / c+10 / c+20 in order of appearance.
fn points(r: &Rec, c: i32) -> Vec<[f64; 3]> {
    let mut out: Vec<[f64; 3]> = vec![];
    for (k, s) in &r.g {
        let f: f64 = s.trim().parse().unwrap_or(0.0);
        if *k == c {
            out.push([f, 0.0, 0.0]);
        } else if *k == c + 10 {
            if let Some(p) = out.last_mut() {
                p[1] = f;
            }
        } else if *k == c + 20 {
            if let Some(p) = out.last_mut() {
                p[2] = f;
            }
        }
    }
    out
}

fn points2(r: &Rec, c: i32) -> Vec<[f64; 2]> {
    points(r, c).into_iter().map(|p| [p[0], p[1]]).collect()
}

/// Today as a Julian day and milliseconds (a fixed date where the clock is not available).
fn julian_now() -> (i32, i32) {
    #[cfg(not(target_arch = "wasm32"))]
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    #[cfg(target_arch = "wasm32")]
    let secs = 1_700_000_000u64;
    ((2_440_588 + secs / 86_400) as i32, ((secs % 86_400) * 1000) as i32)
}

fn aux_header(julian: (i32, i32), handseed: u64) -> Vec<u8> {
    let mut b = vec![0xFF, 0x77, 0x01];
    let rs = |b: &mut Vec<u8>, v: i16| b.extend_from_slice(&v.to_le_bytes());
    let rl = |b: &mut Vec<u8>, v: i32| b.extend_from_slice(&v.to_le_bytes());
    rs(&mut b, 23); // AC1015
    rs(&mut b, 15); // maintenance version
    rl(&mut b, 1); // saves
    rl(&mut b, -1);
    rs(&mut b, 1);
    rs(&mut b, 0);
    rl(&mut b, 0);
    for _ in 0..2 {
        rs(&mut b, 23);
        rs(&mut b, 15);
    }
    for v in [5, 0x893, 5, 0x893, 0, 1] {
        rs(&mut b, v);
    }
    for _ in 0..5 {
        rl(&mut b, 0);
    }
    for _ in 0..2 {
        rl(&mut b, julian.0);
        rl(&mut b, julian.1);
    }
    rl(&mut b, if handseed <= 0x7FFF_FFFF { handseed as i32 } else { -1 });
    rl(&mut b, 0);
    rs(&mut b, 0);
    rs(&mut b, 1);
    for _ in 0..3 {
        rl(&mut b, 0);
    }
    rl(&mut b, 1);
    for _ in 0..4 {
        rl(&mut b, 0);
    }
    b
}

/// An empty preview image section.
fn preview() -> Vec<u8> {
    let mut b = vec![0x1F, 0x25, 0x6D, 0x07, 0xD4, 0x36, 0x28, 0x28, 0x9D, 0x57, 0xCA, 0x3F, 0x9D, 0x44, 0x10, 0x2B];
    b.extend_from_slice(&1u32.to_le_bytes());
    b.push(0);
    b.extend_from_slice(&[0xE0, 0xDA, 0x92, 0xF8, 0x2B, 0xC9, 0xD7, 0xD7, 0x62, 0xA8, 0x35, 0xC0, 0x62, 0xBB, 0xEF, 0xD4]);
    b
}
