//! Reading the objects of a DWG file into DXF-like records: each record has its DXF type, handle, owner, the common
//! entity properties and its groups, with references to other objects kept as handles until every object is known.
//! The field order of each object type is ported from ACadSharp's `DwgObjectReader` (MIT).

use super::bits::{Bits, R};
use super::Ver;
use std::collections::HashMap;

/// A group value.
#[derive(Debug, Clone)]
pub enum V {
    I(i64),
    F(f64),
    S(String),
    /// A reference written as the referenced object's name (block, text style, dimension style).
    Name(u64),
    /// A reference written as a hexadecimal handle.
    Hex(u64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Col {
    ByLayer,
    ByBlock,
    Aci(i16),
    Rgb(u32),
    /// A colour-book colour: the DBCOLOR object with this handle holds the true colour.
    Book(u64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lt {
    ByLayer,
    ByBlock,
    Continuous,
    H(u64),
}

#[derive(Debug, Clone)]
pub struct EntCommon {
    /// 0: owner given (block definitions, vertices, attributes), 1: paper space, 2: model space.
    pub mode: u8,
    pub layer: u64,
    pub lt: Lt,
    pub color: Col,
    pub lw: i16,
    pub ltscale: f64,
    pub invisible: bool,
}

#[derive(Debug, Clone)]
pub enum Info {
    None,
    BlockRecord { name: String, begin: u64, layout: u64, base: [f64; 3] },
    Block { name: String },
    Layer { name: String, color: Col, frozen: bool, off: bool, locked: bool, plot: bool, lw: i16, lt: u64 },
    Ltype { name: String, desc: String, dashes: Vec<f64>, complex: bool, total: f64 },
    Style { name: String, height: f64, width: f64, font: String, bigfont: String },
    /// A dimension style: its name and arrowhead size (DIMASZ x DIMSCALE, when it could be read).
    DimStyle { name: String, arrow: Option<f64> },
    Layout { name: String, tab: i32, block: u64, paper_w: f64, paper_h: f64, units: i16, rot: i16, paper: String },
    ImageDef { path: String, w: f64, h: f64 },
    DbColor(Col),
    /// A dictionary's entries (name, object handle); empty when they could not be read.
    Dict(Vec<(String, u64)>),
    /// A group: its entities.
    Group(Vec<u64>),
    /// A registered application (extended data names its application by this record).
    AppId(String),
}

#[derive(Debug, Clone)]
pub struct Obj {
    pub ty: &'static str,
    pub handle: u64,
    pub owner: u64,
    pub ent: Option<EntCommon>,
    pub g: Vec<(i32, V)>,
    pub info: Info,
}

/// Object type codes below 500 are fixed; the rest are class numbers.
fn fixed_type(t: u16) -> Option<&'static str> {
    Some(match t {
        1 => "TEXT",
        2 => "ATTRIB",
        3 => "ATTDEF",
        4 => "BLOCK",
        5 => "ENDBLK",
        6 => "SEQEND",
        7 => "INSERT",
        8 => "MINSERT",
        10 => "VERTEX2D",
        11 => "VERTEX3D",
        15 => "POLYLINE2D",
        16 => "POLYLINE3D",
        17 => "ARC",
        18 => "CIRCLE",
        19 => "LINE",
        20 => "DIM_ORDINATE",
        21 => "DIM_LINEAR",
        22 => "DIM_ALIGNED",
        23 => "DIM_ANG3PT",
        24 => "DIM_ANG2LN",
        25 => "DIM_RADIUS",
        26 => "DIM_DIAMETER",
        27 => "POINT",
        28 => "3DFACE",
        31 => "SOLID",
        32 => "TRACE",
        34 => "VIEWPORT",
        35 => "ELLIPSE",
        36 => "SPLINE",
        37 => "REGION",
        38 => "3DSOLID",
        40 => "RAY",
        41 => "XLINE",
        42 => "DICTIONARY",
        44 => "MTEXT",
        45 => "LEADER",
        47 => "MLINE",
        49 => "BLOCK_HEADER",
        51 => "LAYER",
        53 => "STYLE",
        57 => "LTYPE",
        67 => "APPID",
        69 => "DIMSTYLE",
        72 => "GROUP",
        77 => "LWPOLYLINE",
        78 => "HATCH",
        82 => "LAYOUT",
        _ => return None,
    })
}

/// The class-based types we read, by their DXF name.
fn class_type(name: &str) -> Option<&'static str> {
    Some(match name {
        "LWPOLYLINE" | "LWPLINE" => "LWPOLYLINE",
        "HATCH" => "HATCH",
        "IMAGE" => "IMAGE",
        "WIPEOUT" => "WIPEOUT",
        "IMAGEDEF" => "IMAGEDEF",
        "LAYOUT" => "LAYOUT",
        "DBCOLOR" => "DBCOLOR",
        "MULTILEADER" => "MULTILEADER",
        _ => return None,
    })
}

struct O<'a> {
    m: Bits<'a>,
    t: Option<Bits<'a>>,
    h: Bits<'a>,
    ver: Ver,
    start: usize,
    handle: u64,
    /// Extended data blocks: the application (APPID handle) and its encoded items.
    eed: Vec<(u64, Vec<u8>)>,
}

impl O<'_> {
    fn tv(&mut self) -> R<String> {
        match self.t.as_mut() {
            Some(t) => t.tv(),
            None => self.m.tv(),
        }
    }
    fn hr(&mut self) -> R<u64> {
        self.h.handle(self.handle)
    }
    /// RL "size in bits" (R2000-R2007, or R13-R14 at their later spot): the handles follow the object data.
    fn update_handles(&mut self) -> R<()> {
        let size = self.m.rl()? as usize;
        self.h.pos = self.start + size;
        if self.ver == Ver::R2007 {
            let mut t = Bits::new(self.m.d, self.ver);
            self.t = if t.set_string_stream((self.start + size) as i64 - 1)? { Some(t) } else { None };
        }
        Ok(())
    }
    fn common(&mut self) -> R<()> {
        if self.ver >= Ver::R2000 && self.ver < Ver::R2010 {
            self.update_handles()?;
        }
        self.handle = self.m.handle(0)?;
        // extended data, by application
        loop {
            let size = self.m.bs()?;
            if size <= 0 {
                break;
            }
            let app = self.m.handle(0)?;
            let b = self.m.bytes(size as u16 as usize)?;
            self.eed.push((app, b));
        }
        Ok(())
    }
    fn reactors(&mut self) -> R<()> {
        let n = self.m.bl()?;
        if !(0..100_000).contains(&n) {
            return Err("bad reactor count".into());
        }
        for _ in 0..n {
            self.hr()?;
        }
        let missing = self.ver >= Ver::R2004 && self.m.bit()?;
        if !missing {
            self.hr()?;
        }
        if self.ver >= Ver::R2013 {
            self.m.bit()?;
        }
        Ok(())
    }
    fn non_entity(&mut self) -> R<u64> {
        self.common()?;
        if self.ver <= Ver::R14 {
            self.update_handles()?;
        }
        let owner = self.hr()?;
        self.reactors()?;
        Ok(owner)
    }
    fn cmc(&mut self) -> R<Col> {
        let idx = self.m.bs()?;
        if self.ver < Ver::R2004 {
            return Ok(aci(idx));
        }
        let rgb = self.m.bl()? as u32;
        let flags = self.m.rc()?;
        if flags & 1 != 0 {
            self.tv()?;
        }
        if flags & 2 != 0 {
            self.tv()?;
        }
        let _ = idx;
        Ok(if rgb == 0xC000_0000 {
            Col::ByLayer
        } else if rgb & 0x0100_0000 != 0 {
            aci((rgb & 0xFF) as i16)
        } else {
            Col::Rgb(rgb & 0xFF_FFFF)
        })
    }
    fn entity(&mut self) -> R<(u64, EntCommon)> {
        self.common()?;
        if self.m.bit()? {
            let n = if self.ver >= Ver::R2010 { self.m.bll()? as usize } else { self.m.rl()? as usize };
            self.m.skip_bytes(n)?;
        }
        if self.ver <= Ver::R14 {
            self.update_handles()?;
        }
        self.entity_mode()
    }
    /// Entity mode, owner, reactors, layer, linetype, colour, linetype scale, visibility and lineweight.
    fn entity_mode(&mut self) -> R<(u64, EntCommon)> {
        let mode = self.m.bb()?;
        let mut owner = 0;
        if mode == 0 {
            owner = self.h.handle(self.handle)?;
        }
        self.reactors()?;
        let mut layer = 0;
        let mut lt = Lt::ByLayer;
        if self.ver <= Ver::R14 {
            layer = self.hr()?;
            if !self.m.bit()? {
                lt = Lt::H(self.hr()?);
            }
        }
        if self.ver < Ver::R2004 && !self.m.bit()? {
            self.hr()?;
            self.hr()?;
        }
        let color = if self.ver >= Ver::R2004 {
            let size = self.m.bs()? as u16;
            if size == 0 {
                Col::ByBlock
            } else {
                let flags = size & 0xFF00;
                let mut c = if flags & 0x4000 != 0 {
                    Col::ByBlock
                } else if flags & 0x8000 != 0 {
                    Col::Rgb(self.m.bl()? as u32 & 0xFF_FFFF)
                } else {
                    aci((size & 0x0FFF) as i16)
                };
                if flags & 0x2000 != 0 {
                    self.m.bl()?;
                }
                if flags & 0x4000 != 0 {
                    c = Col::Book(self.hr()?);
                }
                c
            }
        } else {
            aci(self.m.bs()?)
        };
        let ltscale = self.m.bd()?;
        if self.ver < Ver::R2000 {
            let invisible = self.m.bs()? & 1 == 1;
            return Ok((owner, EntCommon { mode, layer, lt, color, lw: -1, ltscale, invisible }));
        }
        layer = self.hr()?;
        lt = match self.m.bb()? {
            0 => Lt::ByLayer,
            1 => Lt::ByBlock,
            2 => Lt::Continuous,
            _ => Lt::H(self.hr()?),
        };
        if self.ver >= Ver::R2007 {
            if self.m.bb()? == 3 {
                self.hr()?;
            }
            self.m.rc()?;
        }
        if self.m.bb()? == 3 {
            self.hr()?;
        }
        if self.ver >= Ver::R2010 {
            for _ in 0..3 {
                if self.m.bit()? {
                    self.hr()?;
                }
            }
        }
        let invisible = self.m.bs()? & 1 == 1;
        let lw = lineweight(self.m.rc()?);
        Ok((owner, EntCommon { mode, layer, lt, color, lw, ltscale, invisible }))
    }
    fn xref_bits(&mut self) -> R<()> {
        if self.ver >= Ver::R2007 {
            self.m.bs()?;
        } else {
            self.m.bit()?;
            self.m.bs()?;
            self.m.bit()?;
        }
        Ok(())
    }
}

fn aci(i: i16) -> Col {
    match i {
        0 => Col::ByBlock,
        256 => Col::ByLayer,
        i if (1..=255).contains(&i) => Col::Aci(i),
        i if (-255..0).contains(&i) => Col::Aci(-i),
        _ => Col::ByLayer,
    }
}

/// Lineweight byte to hundredths of a millimetre (-1 by layer, -2 by block, -3 default).
pub fn lineweight(b: u8) -> i16 {
    const W: [i16; 24] = [0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200, 211];
    match b {
        28 | 29 => -1,
        30 => -2,
        31 => -3,
        b if (b as usize) < W.len() => W[b as usize],
        _ => -3,
    }
}

fn deg(r: f64) -> f64 {
    r.to_degrees()
}

fn p3(g: &mut Vec<(i32, V)>, code: i32, p: [f64; 3]) {
    g.push((code, V::F(p[0])));
    g.push((code + 10, V::F(p[1])));
    g.push((code + 20, V::F(p[2])));
}

fn p2(g: &mut Vec<(i32, V)>, code: i32, p: [f64; 2]) {
    g.push((code, V::F(p[0])));
    g.push((code + 10, V::F(p[1])));
}

fn ext(g: &mut Vec<(i32, V)>, n: [f64; 3]) {
    if (n[2] - 1.0).abs() > 1e-12 || n[0] != 0.0 || n[1] != 0.0 {
        p3(g, 210, n);
    }
}

/// The type code of the object at `offset`, and where its data starts.
fn peek_type(d: &[u8], ver: Ver, offset: usize) -> Option<u16> {
    let mut m = Bits::at(d, ver, offset * 8);
    m.ms().ok()?;
    if ver >= Ver::R2010 {
        m.mc().ok()?;
    }
    m.ot().ok()
}

/// The DXF-ish name of the object at `offset` (for reports).
pub fn type_name(d: &[u8], ver: Ver, offset: usize, classes: &HashMap<u16, String>) -> Option<String> {
    let t = peek_type(d, ver, offset)?;
    if t < 500 {
        fixed_type(t).map(str::to_string).or_else(|| Some(format!("type {t}")))
    } else {
        classes.get(&t).cloned()
    }
}

/// For an object we do not read: its name if it is an entity (so the drawing can report it as skipped).
pub fn entity_type_name(d: &[u8], ver: Ver, offset: usize, classes: &HashMap<u16, (String, bool)>) -> Option<String> {
    let t = peek_type(d, ver, offset)?;
    match t {
        33 => Some("SHAPE".into()),
        39 => Some("BODY".into()),
        43 | 74 => Some("OLEFRAME".into()),
        46 => Some("TOLERANCE".into()),
        12..=14 => None, // mesh / face vertices (their polyline is reported)
        29 => Some("POLYFACE MESH".into()),
        30 => Some("POLYGON MESH".into()),
        0x1F2 => Some("ACAD_PROXY_ENTITY".into()),
        t if t >= 500 => classes.get(&t).filter(|(_, ent)| *ent).map(|(n, _)| n.clone()),
        _ => None,
    }
}

/// Read the object at byte `offset` of `d`. `Ok(None)` for object types CadForge has no use for.
pub fn read_object(d: &[u8], ver: Ver, offset: usize, classes: &HashMap<u16, String>) -> R<Option<Obj>> {
    let mut m = Bits::at(d, ver, offset * 8);
    let size = m.ms()? as usize;
    if size == 0 {
        return Ok(None);
    }
    let (start, ty, t, h);
    if ver >= Ver::R2010 {
        let hsize = m.mc()? as usize;
        start = m.pos;
        let hoff = (start + size * 8).checked_sub(hsize).ok_or("bad object size")?;
        ty = m.ot()?;
        let mut tb = Bits::new(d, ver);
        t = if tb.set_string_stream(hoff as i64 - 1)? { Some(tb) } else { None };
        h = Bits::at(d, ver, hoff);
    } else {
        start = m.pos;
        ty = m.ot()?;
        t = None;
        h = Bits::at(d, ver, 0);
    }
    let name: &'static str = if ty < 500 {
        match fixed_type(ty) {
            Some(n) => n,
            None => return Ok(None),
        }
    } else {
        match classes.get(&ty).and_then(|n| class_type(n)) {
            Some(n) => n,
            None => return Ok(None),
        }
    };
    let mut o = O { m, t, h, ver, start, handle: 0, eed: vec![] };
    read_typed(&mut o, name).map(Some)
}

fn read_typed(o: &mut O, name: &'static str) -> R<Obj> {
    let ver = o.ver;
    let mut g: Vec<(i32, V)> = vec![];
    // ---- non-graphical objects
    match name {
        "BLOCK_HEADER" => {
            let owner = o.non_entity()?;
            let mut bname = o.tv()?;
            o.xref_bits()?;
            let anon = o.m.bit()?;
            o.m.bit()?; // has attributes
            let xref = o.m.bit()?;
            let overlay = o.m.bit()?;
            if ver >= Ver::R2000 {
                o.m.bit()?;
            }
            let mut owned = 0;
            if ver >= Ver::R2004 && !xref && !overlay {
                owned = o.m.bl()?.clamp(0, 10_000_000);
            }
            let base = o.m.bd3()?;
            o.tv()?;
            let mut inserts = 0;
            if ver >= Ver::R2000 {
                while o.m.rc()? != 0 {
                    inserts += 1;
                }
                o.tv()?;
                let n = o.m.bl()?.max(0) as usize;
                o.m.skip_bytes(n)?;
            }
            if ver >= Ver::R2007 {
                o.m.bs()?;
                o.m.bit()?;
                o.m.rc()?;
            }
            o.hr()?;
            let begin = o.hr()?;
            if ver <= Ver::R2000 && !xref && !overlay {
                o.hr()?;
                o.hr()?;
            }
            for _ in 0..owned {
                o.hr()?;
            }
            o.hr()?; // ENDBLK
            let mut layout = 0;
            if ver >= Ver::R2000 {
                for _ in 0..inserts {
                    o.hr()?;
                }
                layout = o.hr()?;
            }
            if anon && bname.is_empty() {
                bname = "*".into();
            }
            return Ok(Obj { ty: "BLOCK_RECORD", handle: o.handle, owner, ent: None, g, info: Info::BlockRecord { name: bname, begin, layout, base } });
        }
        "LAYER" => {
            let owner = o.non_entity()?;
            let lname = o.tv()?;
            o.xref_bits()?;
            let (frozen, off, locked, plot, lw) = if ver <= Ver::R14 {
                let frozen = o.m.bit()?;
                let off = o.m.bit()?;
                o.m.bit()?;
                (frozen, off, o.m.bit()?, true, -3)
            } else {
                let v = o.m.bs()? as u16;
                (v & 1 != 0, v & 2 != 0, v & 8 != 0, v & 16 != 0, lineweight(((v & 0x3E0) >> 5) as u8))
            };
            let color = o.cmc()?;
            o.hr()?; // xref block
            if ver >= Ver::R2000 {
                o.hr()?; // plot style
            }
            if ver >= Ver::R2007 {
                o.hr()?; // material
            }
            let lt = o.hr()?;
            return Ok(Obj { ty: "LAYER", handle: o.handle, owner, ent: None, g, info: Info::Layer { name: lname, color, frozen, off, locked, plot, lw, lt } });
        }
        "LTYPE" => {
            let owner = o.non_entity()?;
            let lname = o.tv()?;
            o.xref_bits()?;
            let desc = o.tv()?;
            let total = o.m.bd()?;
            o.m.rc()?;
            let n = o.m.rc()? as usize;
            let mut dashes = vec![];
            let mut complex = false;
            let mut is_text = false;
            for _ in 0..n {
                dashes.push(o.m.bd()?);
                o.m.bs()?;
                o.m.rd()?;
                o.m.rd()?;
                o.m.bd()?;
                o.m.bd()?;
                let flags = o.m.bs()?;
                if flags != 0 {
                    complex = true;
                }
                if flags & 2 != 0 {
                    is_text = true;
                }
            }
            if ver <= Ver::R2004 {
                o.m.skip_bytes(256)?;
            }
            if ver >= Ver::R2007 && is_text {
                o.m.skip_bytes(512)?;
            }
            return Ok(Obj { ty: "LTYPE", handle: o.handle, owner, ent: None, g, info: Info::Ltype { name: lname, desc, dashes, complex, total } });
        }
        "STYLE" => {
            let owner = o.non_entity()?;
            let sname = o.tv()?;
            o.xref_bits()?;
            o.m.bit()?;
            o.m.bit()?;
            let height = o.m.bd()?;
            let width = o.m.bd()?;
            o.m.bd()?;
            o.m.rc()?;
            o.m.bd()?;
            let font = o.tv()?;
            let bigfont = o.tv()?;
            return Ok(Obj { ty: "STYLE", handle: o.handle, owner, ent: None, g, info: Info::Style { name: sname, height, width, font, bigfont } });
        }
        "DIMSTYLE" => {
            let owner = o.non_entity()?;
            let sname = o.tv()?;
            let arrow = dimstyle_arrow(o).ok();
            return Ok(Obj { ty: "DIMSTYLE", handle: o.handle, owner, ent: None, g, info: Info::DimStyle { name: sname, arrow } });
        }
        "LAYOUT" => {
            let owner = o.non_entity()?;
            o.tv()?; // page setup name
            o.tv()?; // printer
            o.m.bs()?;
            for _ in 0..4 {
                o.m.bd()?;
            }
            let paper_w = o.m.bd()?;
            let paper_h = o.m.bd()?;
            let paper = o.tv()?;
            o.m.bd()?;
            o.m.bd()?;
            let units = o.m.bs()?;
            let rot = o.m.bs()?;
            o.m.bs()?;
            for _ in 0..4 {
                o.m.bd()?;
            }
            if ver <= Ver::R2000 {
                o.tv()?;
            }
            o.m.bd()?;
            o.m.bd()?;
            o.tv()?; // style sheet
            o.m.bs()?;
            o.m.bd()?;
            o.m.bd2()?;
            if ver >= Ver::R2004 {
                o.m.bs()?;
                o.m.bs()?;
                o.m.bs()?;
                o.hr()?;
            }
            if ver >= Ver::R2007 {
                o.hr()?;
            }
            let lname = o.tv()?;
            let tab = o.m.bl()?;
            o.m.bs()?;
            o.m.bd3()?;
            o.m.rd2()?;
            o.m.rd2()?;
            o.m.bd3()?;
            o.m.bd3()?;
            o.m.bd3()?;
            o.m.bd()?;
            o.m.bs()?;
            o.m.bd3()?;
            o.m.bd3()?;
            if ver >= Ver::R2004 {
                o.m.bl()?;
            }
            let block = o.hr()?;
            return Ok(Obj { ty: "LAYOUT", handle: o.handle, owner, ent: None, g, info: Info::Layout { name: lname, tab, block, paper_w, paper_h, units, rot, paper } });
        }
        "IMAGEDEF" => {
            let owner = o.non_entity()?;
            o.m.bl()?;
            let size = o.m.rd2()?;
            let path = o.tv()?;
            return Ok(Obj { ty: "IMAGEDEF", handle: o.handle, owner, ent: None, g, info: Info::ImageDef { path, w: size[0], h: size[1] } });
        }
        "DBCOLOR" => {
            let owner = o.non_entity()?;
            let idx = o.m.bs()?;
            let col = if ver >= Ver::R2004 {
                let rgb = o.m.bl()? as u32;
                let flags = o.m.rc()?;
                if flags & 1 != 0 {
                    o.tv()?;
                }
                if flags & 2 != 0 {
                    o.tv()?;
                }
                Col::Rgb(rgb & 0xFF_FFFF)
            } else {
                aci(idx)
            };
            return Ok(Obj { ty: "DBCOLOR", handle: o.handle, owner, ent: None, g, info: Info::DbColor(col) });
        }
        "DICTIONARY" => {
            let owner = o.non_entity()?;
            let mut entries = || -> R<Vec<(String, u64)>> {
                let n = o.m.bl()?;
                if !(0..1_000_000).contains(&n) {
                    return Err("bad dictionary size".into());
                }
                if ver == Ver::R14 {
                    o.m.rc()?;
                }
                if ver >= Ver::R2000 {
                    o.m.bs()?; // cloning
                    o.m.rc()?; // hard owner
                }
                let mut out = vec![];
                for _ in 0..n {
                    let name = o.tv()?;
                    out.push((name, o.hr()?));
                }
                Ok(out)
            };
            let info = Info::Dict(entries().unwrap_or_default());
            return Ok(Obj { ty: "DICTIONARY", handle: o.handle, owner, ent: None, g, info });
        }
        "APPID" => {
            let owner = o.non_entity()?;
            let name = o.tv()?;
            return Ok(Obj { ty: "APPID", handle: o.handle, owner, ent: None, g, info: Info::AppId(name) });
        }
        "GROUP" => {
            let owner = o.non_entity()?;
            o.tv()?; // description
            o.m.bs()?; // unnamed
            o.m.bs()?; // selectable
            let n = o.m.bl()?;
            if !(0..10_000_000).contains(&n) {
                return Err("bad group size".into());
            }
            let members = (0..n).map(|_| o.hr()).collect::<R<Vec<u64>>>()?;
            return Ok(Obj { ty: "GROUP", handle: o.handle, owner, ent: None, g, info: Info::Group(members) });
        }
        _ => {}
    }

    // ---- entities
    let (owner, ent) = o.entity()?;
    let mut info = Info::None;
    let ty: &'static str = match name {
        "LINE" => {
            let (a, b);
            if ver <= Ver::R14 {
                a = o.m.bd3()?;
                b = o.m.bd3()?;
            } else {
                let zero = o.m.bit()?;
                let sx = o.m.rd()?;
                let ex = o.m.dd(sx)?;
                let sy = o.m.rd()?;
                let ey = o.m.dd(sy)?;
                let (mut sz, mut ez) = (0.0, 0.0);
                if !zero {
                    sz = o.m.rd()?;
                    ez = o.m.dd(sz)?;
                }
                a = [sx, sy, sz];
                b = [ex, ey, ez];
            }
            p3(&mut g, 10, a);
            p3(&mut g, 11, b);
            g.push((39, V::F(o.m.bt()?)));
            ext(&mut g, o.m.be()?);
            "LINE"
        }
        "CIRCLE" | "ARC" => {
            p3(&mut g, 10, o.m.bd3()?);
            g.push((40, V::F(o.m.bd()?.max(1e-12))));
            g.push((39, V::F(o.m.bt()?)));
            ext(&mut g, o.m.be()?);
            if name == "ARC" {
                g.push((50, V::F(deg(o.m.bd()?))));
                g.push((51, V::F(deg(o.m.bd()?))));
            }
            name
        }
        "POINT" => {
            p3(&mut g, 10, o.m.bd3()?);
            g.push((39, V::F(o.m.bt()?)));
            ext(&mut g, o.m.be()?);
            g.push((50, V::F(deg(o.m.bd()?))));
            "POINT"
        }
        "ELLIPSE" => {
            p3(&mut g, 10, o.m.bd3()?);
            p3(&mut g, 11, o.m.bd3()?);
            ext(&mut g, o.m.bd3()?);
            g.push((40, V::F(o.m.bd()?)));
            g.push((41, V::F(o.m.bd()?)));
            g.push((42, V::F(o.m.bd()?)));
            "ELLIPSE"
        }
        "RAY" | "XLINE" => {
            p3(&mut g, 10, o.m.bd3()?);
            p3(&mut g, 11, o.m.bd3()?);
            name
        }
        "SOLID" | "TRACE" => {
            let th = o.m.bt()?;
            let z = o.m.bd()?;
            for k in 0..4 {
                let p = o.m.rd2()?;
                p3(&mut g, 10 + k, [p[0], p[1], z]);
            }
            g.push((39, V::F(th)));
            ext(&mut g, o.m.be()?);
            name
        }
        "3DFACE" => {
            let mut flags = 0;
            if ver <= Ver::R14 {
                for k in 0..4 {
                    p3(&mut g, 10 + k, o.m.bd3()?);
                }
                flags = o.m.bs()?;
            } else {
                let no_flags = o.m.bit()?;
                let zzero = o.m.bit()?;
                let x = o.m.rd()?;
                let y = o.m.rd()?;
                let z = if zzero { 0.0 } else { o.m.rd()? };
                let mut prev = [x, y, z];
                p3(&mut g, 10, prev);
                for k in 1..4 {
                    prev = o.m.dd3(prev)?;
                    p3(&mut g, 10 + k, prev);
                }
                if !no_flags {
                    flags = o.m.bs()?;
                }
            }
            g.push((70, V::I(flags as i64)));
            "3DFACE"
        }
        "TEXT" | "ATTRIB" | "ATTDEF" => {
            read_text_common(o, &mut g, name)?;
            if name != "TEXT" {
                read_att_common(o, &mut g, name)?;
            }
            name
        }
        "MTEXT" => {
            read_mtext(o, &mut g)?;
            "MTEXT"
        }
        "BLOCK" => {
            let bname = o.tv()?;
            info = Info::Block { name: bname };
            "BLOCK"
        }
        "ENDBLK" => "ENDBLK",
        "SEQEND" => "SEQEND",
        "INSERT" | "MINSERT" => {
            p3(&mut g, 10, o.m.bd3()?);
            let s = if ver <= Ver::R14 {
                o.m.bd3()?
            } else {
                match o.m.bb()? {
                    0 => {
                        let x = o.m.rd()?;
                        [x, o.m.dd(x)?, o.m.dd(x)?]
                    }
                    1 => [1.0, o.m.dd(1.0)?, o.m.dd(1.0)?],
                    2 => {
                        let x = o.m.rd()?;
                        [x, x, x]
                    }
                    _ => [1.0, 1.0, 1.0],
                }
            };
            g.push((41, V::F(s[0])));
            g.push((42, V::F(s[1])));
            g.push((43, V::F(s[2])));
            g.push((50, V::F(deg(o.m.bd()?))));
            ext(&mut g, o.m.bd3()?);
            let has_atts = o.m.bit()?;
            let mut owned = 0;
            if ver >= Ver::R2004 && has_atts {
                owned = o.m.bl()?.clamp(0, 1_000_000);
            }
            if name == "MINSERT" {
                g.push((70, V::I(o.m.bs()? as i64)));
                g.push((71, V::I(o.m.bs()? as i64)));
                g.push((44, V::F(o.m.bd()?)));
                g.push((45, V::F(o.m.bd()?)));
            }
            g.push((2, V::Name(o.hr()?)));
            if has_atts {
                g.push((66, V::I(1)));
            }
            let _ = owned;
            "INSERT"
        }
        "VERTEX2D" => {
            let flags = o.m.rc()?;
            p3(&mut g, 10, o.m.bd3()?);
            let w = o.m.bd()?;
            let (sw, ew) = if w < 0.0 { (-w, -w) } else { (w, o.m.bd()?) };
            g.push((40, V::F(sw)));
            g.push((41, V::F(ew)));
            g.push((42, V::F(o.m.bd()?)));
            g.push((70, V::I(flags as i64)));
            "VERTEX"
        }
        "VERTEX3D" => {
            let flags = o.m.rc()?;
            p3(&mut g, 10, o.m.bd3()?);
            g.push((70, V::I(flags as i64 | 32)));
            "VERTEX"
        }
        "POLYLINE2D" => {
            let flags = o.m.bs()?;
            let curve = o.m.bs()?;
            let sw = o.m.bd()?;
            let ew = o.m.bd()?;
            let th = o.m.bt()?;
            let elev = o.m.bd()?;
            let n = o.m.be()?;
            g.push((66, V::I(1)));
            p3(&mut g, 10, [0.0, 0.0, elev]);
            g.push((70, V::I(flags as i64)));
            g.push((40, V::F(sw)));
            g.push((41, V::F(ew)));
            g.push((75, V::I(curve as i64)));
            g.push((39, V::F(th)));
            ext(&mut g, n);
            "POLYLINE"
        }
        "POLYLINE3D" => {
            let f1 = o.m.rc()?;
            let f2 = o.m.rc()?;
            let mut flags = 8;
            if f1 & 3 != 0 {
                flags |= 4;
            }
            if f2 & 1 != 0 {
                flags |= 1;
            }
            g.push((66, V::I(1)));
            p3(&mut g, 10, [0.0; 3]);
            g.push((70, V::I(flags)));
            "POLYLINE"
        }
        "LWPOLYLINE" => {
            read_lwpolyline(o, &mut g)?;
            "LWPOLYLINE"
        }
        "SPLINE" => {
            read_spline(o, &mut g)?;
            "SPLINE"
        }
        "HATCH" => {
            read_hatch(o, &mut g)?;
            "HATCH"
        }
        "IMAGE" | "WIPEOUT" => {
            read_image(o, &mut g)?;
            name
        }
        "VIEWPORT" => {
            read_viewport(o, &mut g)?;
            "VIEWPORT"
        }
        "LEADER" => {
            read_leader(o, &mut g)?;
            "LEADER"
        }
        "MLINE" => {
            read_mline(o, &mut g)?;
            "MLINE"
        }
        "MULTILEADER" => {
            read_multileader(o, &mut g)?;
            "MULTILEADER"
        }
        "DIM_ORDINATE" | "DIM_LINEAR" | "DIM_ALIGNED" | "DIM_ANG3PT" | "DIM_ANG2LN" | "DIM_RADIUS" | "DIM_DIAMETER" => {
            read_dimension(o, &mut g, name)?;
            "DIMENSION"
        }
        // REGION / 3DSOLID: ACIS data CadForge cannot use; reported as skipped
        "REGION" | "3DSOLID" => name,
        _ => return Err(format!("unhandled {name}")),
    };
    eed_groups(&o.eed, o.ver, &mut g);
    Ok(Obj { ty, handle: o.handle, owner, ent: Some(ent), g, info })
}

/// DIMSCALE x DIMASZ of a dimension style, read after its name.
fn dimstyle_arrow(o: &mut O) -> R<f64> {
    o.xref_bits()?;
    if o.ver <= Ver::R14 {
        for _ in 0..11 {
            o.m.bit()?;
        }
        for k in 0..12 {
            if matches!(k, 2 | 3 | 7) {
                o.m.bit()?;
            } else {
                o.m.rc()?;
            }
        }
        for _ in 0..6 {
            o.m.bs()?;
        }
    } else {
        o.tv()?;
        o.tv()?;
    }
    let scale = o.m.bd()?;
    let size = o.m.bd()?;
    Ok(size * if scale > 0.0 { scale } else { 1.0 })
}

fn read_text_common(o: &mut O, g: &mut Vec<(i32, V)>, name: &str) -> R<()> {
    let ver = o.ver;
    let valign_code = if name == "TEXT" { 73 } else { 74 };
    if ver <= Ver::R14 {
        let z = o.m.bd()?;
        let ins = o.m.rd2()?;
        let al = o.m.rd2()?;
        let n = o.m.bd3()?;
        let th = o.m.bd()?;
        let obl = o.m.bd()?;
        let rot = o.m.bd()?;
        let h = o.m.bd()?;
        let wf = o.m.bd()?;
        let s = o.tv()?;
        let gen = o.m.bs()?;
        let ha = o.m.bs()?;
        let va = o.m.bs()?;
        p3(g, 10, [ins[0], ins[1], z]);
        p3(g, 11, [al[0], al[1], z]);
        ext(g, n);
        g.push((39, V::F(th)));
        g.push((51, V::F(deg(obl))));
        g.push((50, V::F(deg(rot))));
        g.push((40, V::F(h)));
        g.push((41, V::F(wf)));
        g.push((1, V::S(s)));
        g.push((71, V::I(gen as i64)));
        g.push((72, V::I(ha as i64)));
        g.push((valign_code, V::I(va as i64)));
        g.push((7, V::Name(o.hr()?)));
        return Ok(());
    }
    let flags = o.m.rc()?;
    let z = if flags & 1 == 0 { o.m.rd()? } else { 0.0 };
    let ins = o.m.rd2()?;
    p3(g, 10, [ins[0], ins[1], z]);
    if flags & 2 == 0 {
        let x = o.m.dd(ins[0])?;
        let y = o.m.dd(ins[1])?;
        p3(g, 11, [x, y, z]);
    }
    ext(g, o.m.be()?);
    g.push((39, V::F(o.m.bt()?)));
    if flags & 4 == 0 {
        g.push((51, V::F(deg(o.m.rd()?))));
    }
    if flags & 8 == 0 {
        g.push((50, V::F(deg(o.m.rd()?))));
    }
    g.push((40, V::F(o.m.rd()?)));
    if flags & 0x10 == 0 {
        g.push((41, V::F(o.m.rd()?)));
    }
    g.push((1, V::S(o.tv()?)));
    if flags & 0x20 == 0 {
        g.push((71, V::I(o.m.bs()? as i64)));
    }
    if flags & 0x40 == 0 {
        g.push((72, V::I(o.m.bs()? as i64)));
    }
    if flags & 0x80 == 0 {
        g.push((valign_code, V::I(o.m.bs()? as i64)));
    }
    g.push((7, V::Name(o.hr()?)));
    Ok(())
}

fn read_att_common(o: &mut O, g: &mut Vec<(i32, V)>, name: &str) -> R<()> {
    let ver = o.ver;
    if ver >= Ver::R2010 {
        o.m.rc()?;
    }
    let mut kind = 1;
    if ver >= Ver::R2018 {
        kind = o.m.rc()?;
    }
    if kind == 2 || kind == 4 {
        // multi-line attribute: an embedded MTEXT (its entity mode, then the MTEXT fields)
        o.entity_mode()?;
        let mut sub = vec![];
        read_mtext(o, &mut sub)?;
        let n = o.m.bs()?;
        if n > 0 {
            o.m.skip_bytes(n as usize)?;
            o.hr()?;
            o.m.bs()?;
        }
    }
    g.push((2, V::S(o.tv()?)));
    o.m.bs()?;
    g.push((70, V::I(o.m.rc()? as i64)));
    if ver >= Ver::R2007 {
        o.m.bit()?;
    }
    if name == "ATTDEF" {
        if ver >= Ver::R2010 {
            o.m.rc()?;
        }
        g.push((3, V::S(o.tv()?)));
    }
    Ok(())
}

fn read_mtext(o: &mut O, g: &mut Vec<(i32, V)>) -> R<()> {
    let ver = o.ver;
    p3(g, 10, o.m.bd3()?);
    ext(g, o.m.bd3()?);
    p3(g, 11, o.m.bd3()?);
    g.push((41, V::F(o.m.bd()?)));
    if ver >= Ver::R2007 {
        g.push((46, V::F(o.m.bd()?)));
    }
    g.push((40, V::F(o.m.bd()?)));
    g.push((71, V::I(o.m.bs()? as i64)));
    g.push((72, V::I(o.m.bs()? as i64)));
    g.push((43, V::F(o.m.bd()?)));
    g.push((42, V::F(o.m.bd()?)));
    g.push((1, V::S(o.tv()?)));
    g.push((7, V::Name(o.hr()?)));
    if ver >= Ver::R2000 {
        g.push((73, V::I(o.m.bs()? as i64)));
        g.push((44, V::F(o.m.bd()?)));
        o.m.bit()?;
    }
    if ver >= Ver::R2004 {
        let bg = o.m.bl()?;
        g.push((90, V::I(bg as i64)));
        if bg & 1 != 0 || (ver >= Ver::R2018 && bg & 0x10 != 0) {
            g.push((45, V::F(o.m.bd()?)));
            o.cmc()?;
            o.m.bl()?;
        }
    }
    if ver >= Ver::R2018 && o.m.bit()? {
        // not annotative: redundant copies of the fields above, then column data
        o.m.bs()?;
        o.m.bit()?;
        o.hr()?;
        o.m.bl()?;
        o.m.bd3()?;
        o.m.bd3()?;
        for _ in 0..4 {
            o.m.bd()?;
        }
        let ctype = o.m.bs()?;
        if ctype != 0 {
            let count = o.m.bl()?.clamp(0, 10_000);
            o.m.bd()?;
            o.m.bd()?;
            let auto = o.m.bit()?;
            o.m.bit()?;
            if !auto && ctype == 2 {
                for _ in 0..count {
                    o.m.bd()?;
                }
            }
        }
    }
    Ok(())
}

fn read_lwpolyline(o: &mut O, g: &mut Vec<(i32, V)>) -> R<()> {
    let ver = o.ver;
    let flags = o.m.bs()? as u16;
    let mut f70 = 0;
    if flags & 0x200 != 0 {
        f70 |= 1;
    }
    if flags & 0x100 != 0 {
        f70 |= 128;
    }
    if flags & 4 != 0 {
        g.push((43, V::F(o.m.bd()?)));
    }
    if flags & 8 != 0 {
        g.push((38, V::F(o.m.bd()?)));
    }
    if flags & 2 != 0 {
        g.push((39, V::F(o.m.bd()?)));
    }
    if flags & 1 != 0 {
        ext(g, o.m.bd3()?);
    }
    let nv = o.m.bl()?.clamp(0, 5_000_000) as usize;
    let nb = if flags & 0x10 != 0 { o.m.bl()?.clamp(0, 5_000_000) as usize } else { 0 };
    let nid = if flags & 0x400 != 0 { o.m.bl()?.clamp(0, 5_000_000) as usize } else { 0 };
    let nw = if flags & 0x20 != 0 { o.m.bl()?.clamp(0, 5_000_000) as usize } else { 0 };
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(nv);
    if ver <= Ver::R14 {
        for _ in 0..nv {
            pts.push(o.m.rd2()?);
        }
    } else if nv > 0 {
        let mut p = o.m.rd2()?;
        pts.push(p);
        for _ in 1..nv {
            p = o.m.dd2(p)?;
            pts.push(p);
        }
    }
    let mut bulges = vec![0.0; nv];
    for k in 0..nb {
        let b = o.m.bd()?;
        if let Some(x) = bulges.get_mut(k) {
            *x = b;
        }
    }
    for _ in 0..nid {
        o.m.bl()?;
    }
    let mut widths = vec![(0.0, 0.0); nv];
    for k in 0..nw {
        let w = (o.m.bd()?, o.m.bd()?);
        if let Some(x) = widths.get_mut(k) {
            *x = w;
        }
    }
    g.push((90, V::I(nv as i64)));
    g.push((70, V::I(f70)));
    for k in 0..nv {
        p2(g, 10, pts[k]);
        if nw > 0 {
            g.push((40, V::F(widths[k].0)));
            g.push((41, V::F(widths[k].1)));
        }
        if bulges[k] != 0.0 {
            g.push((42, V::F(bulges[k])));
        }
    }
    Ok(())
}

fn read_spline(o: &mut O, g: &mut Vec<(i32, V)>) -> R<()> {
    let ver = o.ver;
    let mut scenario = o.m.bl()?;
    let mut flags = 8; // planar
    if ver >= Ver::R2013 {
        let f1 = o.m.bl()?;
        let knot_param = o.m.bl()?;
        if f1 & 4 != 0 {
            flags |= 1;
        }
        scenario = if knot_param == 15 || f1 & 8 == 0 { 1 } else { 2 };
    }
    let degree = o.m.bl()?;
    let (mut nfit, mut nknots, mut nctrl, mut weights) = (0usize, 0usize, 0usize, false);
    let mut tail = vec![];
    match scenario {
        1 => {
            if o.m.bit()? {
                flags |= 4;
            }
            if o.m.bit()? {
                flags |= 1;
            }
            if o.m.bit()? {
                flags |= 2;
            }
            tail.push((42, V::F(o.m.bd()?)));
            tail.push((43, V::F(o.m.bd()?)));
            nknots = o.m.bl()?.clamp(0, 1_000_000) as usize;
            nctrl = o.m.bl()?.clamp(0, 1_000_000) as usize;
            weights = o.m.bit()?;
        }
        2 => {
            tail.push((44, V::F(o.m.bd()?)));
            p3(&mut tail, 12, o.m.bd3()?);
            p3(&mut tail, 13, o.m.bd3()?);
            nfit = o.m.bl()?.clamp(0, 1_000_000) as usize;
        }
        _ => return Err("bad spline".into()),
    }
    if scenario == 2 && nfit == 2 {
        // AutoCAD makes a two-point fit spline into one cubic Bezier: P0, P0 + T0/3, P1 - T1/3, P1
        let t = |c: i32| tail.iter().filter(|(k, _)| *k == c).map(|(_, v)| if let V::F(x) = v { *x } else { 0.0 }).sum::<f64>();
        let (t0, t1) = ([t(12), t(22), t(32)], [t(13), t(23), t(33)]);
        if t0.iter().chain(t1.iter()).any(|v| *v != 0.0) {
            let p0 = o.m.bd3()?;
            let p1 = o.m.bd3()?;
            g.push((70, V::I(flags)));
            g.push((71, V::I(3)));
            g.push((72, V::I(8)));
            g.push((73, V::I(4)));
            g.push((74, V::I(0)));
            for k in [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0] {
                g.push((40, V::F(k)));
            }
            for q in [p0, [p0[0] + t0[0] / 3.0, p0[1] + t0[1] / 3.0, p0[2] + t0[2] / 3.0], [p1[0] - t1[0] / 3.0, p1[1] - t1[1] / 3.0, p1[2] - t1[2] / 3.0], p1] {
                p3(g, 10, q);
            }
            return Ok(());
        }
    }
    g.push((70, V::I(flags)));
    g.push((71, V::I(degree as i64)));
    g.push((72, V::I(nknots as i64)));
    g.push((73, V::I(nctrl as i64)));
    g.push((74, V::I(nfit as i64)));
    g.extend(tail);
    for _ in 0..nknots {
        g.push((40, V::F(o.m.bd()?)));
    }
    for _ in 0..nctrl {
        p3(g, 10, o.m.bd3()?);
        if weights {
            g.push((41, V::F(o.m.bd()?)));
        }
    }
    for _ in 0..nfit {
        p3(g, 11, o.m.bd3()?);
    }
    Ok(())
}

fn read_hatch(o: &mut O, g: &mut Vec<(i32, V)>) -> R<()> {
    let ver = o.ver;
    let mut grad = vec![];
    if ver >= Ver::R2004 {
        let enabled = o.m.bl()? != 0;
        let reserved = o.m.bl()?;
        let angle = o.m.bd()?;
        let shift = o.m.bd()?;
        let single = o.m.bl()?;
        let tint = o.m.bd()?;
        let n = o.m.bl()?.clamp(0, 1000);
        let mut cols = vec![];
        for _ in 0..n {
            let v = o.m.bd()?;
            let c = o.cmc()?;
            cols.push((v, c));
        }
        let gname = o.tv()?;
        if enabled {
            grad.push((450, V::I(1)));
            grad.push((451, V::I(reserved as i64)));
            grad.push((452, V::I(single as i64)));
            grad.push((453, V::I(cols.len() as i64)));
            for (v, c) in cols {
                grad.push((463, V::F(v)));
                let rgb = match c {
                    Col::Rgb(x) => x,
                    Col::Aci(i) => {
                        let [r, gg, b] = crate::style::aci_rgb(i as u8);
                        (r as u32) << 16 | (gg as u32) << 8 | b as u32
                    }
                    _ => 0,
                };
                grad.push((63, V::I(5)));
                grad.push((421, V::I(rgb as i64)));
            }
            grad.push((460, V::F(angle)));
            grad.push((461, V::F(shift)));
            grad.push((462, V::F(tint)));
            grad.push((470, V::S(gname)));
        }
    }
    let z = o.m.bd()?;
    p3(g, 10, [0.0, 0.0, z]);
    ext(g, o.m.bd3()?);
    g.push((2, V::S(o.tv()?)));
    let solid = o.m.bit()?;
    g.push((70, V::I(solid as i64)));
    g.push((71, V::I(o.m.bit()? as i64)));
    let npaths = o.m.bl()?.clamp(0, 100_000);
    g.push((91, V::I(npaths as i64)));
    let mut derived = false;
    for _ in 0..npaths {
        let pflags = o.m.bl()?;
        if pflags & 4 != 0 {
            derived = true;
        }
        g.push((92, V::I(pflags as i64)));
        if pflags & 2 == 0 {
            let nseg = o.m.bl()?.clamp(0, 1_000_000);
            g.push((93, V::I(nseg as i64)));
            for _ in 0..nseg {
                let et = o.m.rc()?;
                g.push((72, V::I(et as i64)));
                match et {
                    1 => {
                        p2(g, 10, o.m.rd2()?);
                        p2(g, 11, o.m.rd2()?);
                    }
                    2 => {
                        p2(g, 10, o.m.rd2()?);
                        g.push((40, V::F(o.m.bd()?)));
                        g.push((50, V::F(deg(o.m.bd()?))));
                        g.push((51, V::F(deg(o.m.bd()?))));
                        g.push((73, V::I(o.m.bit()? as i64)));
                    }
                    3 => {
                        p2(g, 10, o.m.rd2()?);
                        p2(g, 11, o.m.rd2()?);
                        g.push((40, V::F(o.m.bd()?)));
                        g.push((50, V::F(deg(o.m.bd()?))));
                        g.push((51, V::F(deg(o.m.bd()?))));
                        g.push((73, V::I(o.m.bit()? as i64)));
                    }
                    4 => {
                        let degree = o.m.bl()?;
                        let rational = o.m.bit()?;
                        let periodic = o.m.bit()?;
                        let nk = o.m.bl()?.clamp(0, 1_000_000);
                        let nc = o.m.bl()?.clamp(0, 1_000_000);
                        g.push((94, V::I(degree as i64)));
                        g.push((73, V::I(rational as i64)));
                        g.push((74, V::I(periodic as i64)));
                        g.push((95, V::I(nk as i64)));
                        g.push((96, V::I(nc as i64)));
                        for _ in 0..nk {
                            g.push((40, V::F(o.m.bd()?)));
                        }
                        for _ in 0..nc {
                            p2(g, 10, o.m.rd2()?);
                            if rational {
                                g.push((42, V::F(o.m.bd()?)));
                            }
                        }
                        if ver >= Ver::R2010 {
                            let nf = o.m.bl()?.clamp(0, 1_000_000);
                            g.push((97, V::I(nf as i64)));
                            if nf > 0 {
                                for _ in 0..nf {
                                    p2(g, 11, o.m.rd2()?);
                                }
                                p2(g, 12, o.m.rd2()?);
                                p2(g, 13, o.m.rd2()?);
                            }
                        }
                    }
                    _ => return Err("bad hatch edge".into()),
                }
            }
        } else {
            let bulges = o.m.bit()?;
            let closed = o.m.bit()?;
            let n = o.m.bl()?.clamp(0, 1_000_000);
            g.push((72, V::I(bulges as i64)));
            g.push((73, V::I(closed as i64)));
            g.push((93, V::I(n as i64)));
            for _ in 0..n {
                p2(g, 10, o.m.rd2()?);
                if bulges {
                    g.push((42, V::F(o.m.bd()?)));
                }
            }
        }
        let nb = o.m.bl()?.clamp(0, 1_000_000);
        g.push((97, V::I(nb as i64)));
        for _ in 0..nb {
            g.push((330, V::Hex(o.hr()?)));
        }
    }
    g.push((75, V::I(o.m.bs()? as i64)));
    g.push((76, V::I(o.m.bs()? as i64)));
    if !solid {
        g.push((52, V::F(deg(o.m.bd()?))));
        g.push((41, V::F(o.m.bd()?)));
        g.push((77, V::I(o.m.bit()? as i64)));
        let nl = o.m.bs()?.max(0);
        g.push((78, V::I(nl as i64)));
        for _ in 0..nl {
            g.push((53, V::F(deg(o.m.bd()?))));
            let b = o.m.bd2()?;
            g.push((43, V::F(b[0])));
            g.push((44, V::F(b[1])));
            let off = o.m.bd2()?;
            g.push((45, V::F(off[0])));
            g.push((46, V::F(off[1])));
            let nd = o.m.bs()?.max(0);
            g.push((79, V::I(nd as i64)));
            for _ in 0..nd {
                g.push((49, V::F(o.m.bd()?)));
            }
        }
    }
    if derived {
        g.push((47, V::F(o.m.bd()?)));
    }
    let ns = o.m.bl()?.clamp(0, 100_000);
    g.push((98, V::I(ns as i64)));
    for _ in 0..ns {
        p2(g, 10, o.m.rd2()?);
    }
    g.extend(grad);
    Ok(())
}

fn read_image(o: &mut O, g: &mut Vec<(i32, V)>) -> R<()> {
    let ver = o.ver;
    o.m.bl()?;
    p3(g, 10, o.m.bd3()?);
    p3(g, 11, o.m.bd3()?);
    p3(g, 12, o.m.bd3()?);
    p2(g, 13, o.m.rd2()?);
    g.push((70, V::I(o.m.bs()? as i64)));
    g.push((280, V::I(o.m.bit()? as i64)));
    g.push((281, V::I(o.m.rc()? as i64)));
    g.push((282, V::I(o.m.rc()? as i64)));
    g.push((283, V::I(o.m.rc()? as i64)));
    if ver >= Ver::R2010 {
        o.m.bit()?;
    }
    let ct = o.m.bs()?;
    g.push((71, V::I(ct as i64)));
    match ct {
        1 => {
            g.push((91, V::I(2)));
            p2(g, 14, o.m.rd2()?);
            p2(g, 14, o.m.rd2()?);
        }
        2 => {
            let n = o.m.bl()?.clamp(0, 1_000_000);
            g.push((91, V::I(n as i64)));
            for _ in 0..n {
                p2(g, 14, o.m.rd2()?);
            }
        }
        _ => {}
    }
    g.push((340, V::Hex(o.hr()?)));
    Ok(())
}

fn read_viewport(o: &mut O, g: &mut Vec<(i32, V)>) -> R<()> {
    let ver = o.ver;
    p3(g, 10, o.m.bd3()?);
    g.push((40, V::F(o.m.bd()?)));
    g.push((41, V::F(o.m.bd()?)));
    let mut status = 0;
    let mut nfrozen = 0;
    if ver >= Ver::R2000 {
        p3(g, 17, o.m.bd3()?);
        p3(g, 16, o.m.bd3()?);
        g.push((51, V::F(deg(o.m.bd()?))));
        g.push((45, V::F(o.m.bd()?)));
        g.push((42, V::F(o.m.bd()?)));
        g.push((43, V::F(o.m.bd()?)));
        g.push((44, V::F(o.m.bd()?)));
        g.push((50, V::F(deg(o.m.bd()?))));
        p2(g, 12, o.m.rd2()?);
        p2(g, 13, o.m.rd2()?);
        p2(g, 14, o.m.rd2()?);
        p2(g, 15, o.m.rd2()?);
        o.m.bs()?;
    }
    if ver >= Ver::R2007 {
        o.m.bs()?;
    }
    if ver >= Ver::R2000 {
        nfrozen = o.m.bl()?.clamp(0, 100_000);
        status = o.m.bl()?;
        o.tv()?;
        o.m.rc()?;
        o.m.bit()?;
        o.m.bit()?;
        o.m.bd3()?;
        o.m.bd3()?;
        o.m.bd3()?;
        o.m.bd()?;
        o.m.bs()?;
    }
    if ver >= Ver::R2004 {
        o.m.bs()?;
    }
    if ver >= Ver::R2007 {
        o.m.bit()?;
        o.m.rc()?;
        o.m.bd()?;
        o.m.bd()?;
        o.cmc()?;
    }
    g.push((90, V::I(status as i64)));
    g.push((68, V::I(if status & 131072 != 0 { 0 } else { 1 })));
    if ver <= Ver::R14 {
        o.hr()?;
        mview_eed(&o.eed, o.ver, g);
    }
    for _ in 0..nfrozen {
        g.push((331, V::Hex(o.hr()?)));
    }
    Ok(())
}

/// R13/R14 keep a viewport's view in its "MVIEW" extended data: after the version, the view target and direction,
/// then the twist, height and centre of the view (reals), snap and grid settings, and the frozen layers.
fn mview_eed(eed: &[(u64, Vec<u8>)], ver: Ver, g: &mut Vec<(i32, V)>) {
    for (_, b) in eed {
        let items = eed_items(b, ver);
        if !matches!(items.first(), Some((1000, EedV::S(s))) if s == "MVIEW") {
            continue;
        }
        let reals: Vec<f64> = items.iter().filter_map(|(c, v)| if let (1040, EedV::F(x)) = (c, v) { Some(*x) } else { None }).collect();
        let pts: Vec<[f64; 3]> = items.iter().filter_map(|(c, v)| if let (1010, EedV::P(p)) = (c, v) { Some(*p) } else { None }).collect();
        if let ([t, d, ..], [_, h, cx, cy, ..]) = (pts.as_slice(), reals.as_slice()) {
            p3(g, 17, *t);
            p3(g, 16, *d);
            g.push((45, V::F(*h)));
            p2(g, 12, [*cx, *cy]);
        }
        for (c, v) in &items {
            if let (1003, EedV::H(h)) = (c, v) {
                g.push((331, V::Hex(*h)));
            }
        }
        return;
    }
}

enum EedV {
    S(String),
    F(f64),
    P([f64; 3]),
    H(u64),
    Other,
}

/// Extended data strings as DXF groups (1001 application, then its 1000 strings), at the end of an entity's
/// groups; CadForge keeps its own description of some objects there.
fn eed_groups(eed: &[(u64, Vec<u8>)], ver: Ver, g: &mut Vec<(i32, V)>) {
    for (app, b) in eed {
        let strings: Vec<String> = eed_items(b, ver).into_iter().filter_map(|(c, v)| if let (1000, EedV::S(s)) = (c, v) { Some(s) } else { None }).collect();
        if !strings.is_empty() {
            g.push((1001, V::Name(*app)));
            g.extend(strings.into_iter().map(|s| (1000, V::S(s))));
        }
    }
}

/// The items of one extended data block, as (DXF group code, value); stops at anything odd. (Strings are stored as
/// AutoCAD writes them: a two-byte length and a code page byte before the characters.)
fn eed_items(b: &[u8], ver: Ver) -> Vec<(i32, EedV)> {
    let mut out = vec![];
    let mut i = 0;
    let take = |i: &mut usize, n: usize| -> Option<&[u8]> {
        let s = b.get(*i..*i + n)?;
        *i += n;
        Some(s)
    };
    while i < b.len() {
        let code = b[i] as i32;
        i += 1;
        let v = match code {
            0 if ver >= Ver::R2007 => {
                // a character count and UTF-16 characters
                let Some(n) = take(&mut i, 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize) else { break };
                let Some(s) = take(&mut i, 2 * n) else { break };
                EedV::S(char::decode_utf16(s.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]]))).map(|c| c.unwrap_or('?')).collect())
            }
            0 => {
                // a two-byte length, a code page byte, the bytes
                let Some(n) = take(&mut i, 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize) else { break };
                let Some(_) = take(&mut i, 1) else { break };
                let Some(s) = take(&mut i, n) else { break };
                EedV::S(s.iter().map(|&c| c as char).collect())
            }
            2 => {
                let Some(_) = take(&mut i, 1) else { break };
                EedV::Other
            }
            3 | 5 => {
                let Some(s) = take(&mut i, 8) else { break };
                EedV::H(u64::from_be_bytes(s.try_into().unwrap_or([0; 8])))
            }
            4 => {
                let Some(&[n]) = take(&mut i, 1) else { break };
                let Some(_) = take(&mut i, n as usize) else { break };
                EedV::Other
            }
            10..=13 => {
                let Some(s) = take(&mut i, 24) else { break };
                let f = |k: usize| f64::from_le_bytes(s[k * 8..k * 8 + 8].try_into().unwrap_or([0; 8]));
                EedV::P([f(0), f(1), f(2)])
            }
            40..=42 => {
                let Some(s) = take(&mut i, 8) else { break };
                EedV::F(f64::from_le_bytes(s.try_into().unwrap_or([0; 8])))
            }
            70 => {
                let Some(_) = take(&mut i, 2) else { break };
                EedV::Other
            }
            71 => {
                let Some(_) = take(&mut i, 4) else { break };
                EedV::Other
            }
            _ => break,
        };
        out.push((code + 1000, v));
    }
    out
}

fn read_leader(o: &mut O, g: &mut Vec<(i32, V)>) -> R<()> {
    let ver = o.ver;
    o.m.bit()?;
    let creation = o.m.bs()?;
    let path = o.m.bs()?;
    let n = o.m.bl()?.clamp(0, 100_000);
    let mut pts = vec![];
    for _ in 0..n {
        pts.push(o.m.bd3()?);
    }
    o.m.bd3()?;
    let normal = o.m.bd3()?;
    let xdir = o.m.bd3()?;
    o.m.bd3()?;
    if ver >= Ver::R14 {
        o.m.bd3()?;
    }
    if ver <= Ver::R14 {
        o.m.bd()?;
    }
    let mut box_h = 0.0;
    if ver <= Ver::R2007 {
        box_h = o.m.bd()?;
        g.push((41, V::F(o.m.bd()?)));
    }
    if box_h > 0.0 {
        g.push((40, V::F(box_h)));
    }
    let hook_same = o.m.bit()?;
    let arrow = o.m.bit()?;
    if ver <= Ver::R14 {
        for _ in 0..2 {
            o.m.bs()?;
        }
        o.m.bd()?;
        for _ in 0..2 {
            o.m.bit()?;
        }
        o.m.bs()?;
        o.m.bs()?;
        o.m.bit()?;
        o.m.bit()?;
    } else {
        o.m.bs()?;
        o.m.bit()?;
        o.m.bit()?;
    }
    g.push((340, V::Hex(o.hr()?)));
    g.push((3, V::Name(o.hr()?)));
    g.push((71, V::I(arrow as i64)));
    g.push((72, V::I(path as i64)));
    g.push((73, V::I(creation as i64)));
    g.push((74, V::I(hook_same as i64)));
    g.push((76, V::I(pts.len() as i64)));
    for p in pts {
        p3(g, 10, p);
    }
    ext(g, normal);
    p3(g, 211, xdir);
    Ok(())
}

/// A multileader, written as the DXF groups the DXF reader understands: the context's text height and text, then
/// for each leader its landing point, dogleg direction and length, and the vertices of each of its lines.
fn read_multileader(o: &mut O, g: &mut Vec<(i32, V)>) -> R<()> {
    let ver = o.ver;
    if ver >= Ver::R2010 {
        o.m.bs()?;
    }
    // per leader: its groups (landing, dogleg) and the vertices of each of its lines
    type Root = (Vec<(i32, V)>, Vec<Vec<[f64; 3]>>);
    let mut roots: Vec<Root> = vec![];
    let nroots = o.m.bl()?.clamp(0, 10_000);
    for _ in 0..nroots {
        let mut rg = vec![];
        o.m.bit()?;
        o.m.bit()?;
        p3(&mut rg, 10, o.m.bd3()?);
        p3(&mut rg, 11, o.m.bd3()?);
        let breaks = o.m.bl()?.clamp(0, 100_000);
        for _ in 0..breaks {
            o.m.bd3()?;
            o.m.bd3()?;
        }
        o.m.bl()?;
        rg.push((40, V::F(o.m.bd()?)));
        let nlines = o.m.bl()?.clamp(0, 10_000);
        let mut lines = vec![];
        for _ in 0..nlines {
            let n = o.m.bl()?.clamp(0, 100_000);
            let mut pts = vec![];
            for _ in 0..n {
                pts.push(o.m.bd3()?);
            }
            let nb = o.m.bl()?.clamp(0, 100_000);
            for _ in 0..nb {
                o.m.bl()?;
                let k = o.m.bl()?.clamp(0, 100_000);
                for _ in 0..k {
                    o.m.bd3()?;
                    o.m.bd3()?;
                }
            }
            o.m.bl()?;
            if ver >= Ver::R2010 {
                o.m.bs()?;
                o.cmc()?;
                o.hr()?;
                o.m.bl()?;
                o.m.bd()?;
                o.hr()?;
                o.m.bl()?;
            }
            lines.push(pts);
        }
        if ver >= Ver::R2010 {
            o.m.bs()?;
        }
        roots.push((rg, lines));
    }
    o.m.bd()?; // scale
    o.m.bd3()?; // content base point
    let height = o.m.bd()?;
    for _ in 0..2 {
        o.m.bd()?; // arrowhead size, landing gap
    }
    for _ in 0..4 {
        o.m.bs()?;
    }
    let text = if o.m.bit()? { o.tv()? } else { String::new() };
    g.push((300, V::S("CONTEXT_DATA{".into())));
    g.push((41, V::F(height)));
    if !text.is_empty() {
        g.push((304, V::S(text)));
    }
    for (rg, lines) in roots {
        g.push((302, V::S("LEADER{".into())));
        g.extend(rg);
        for pts in lines {
            g.push((304, V::S("LEADER_LINE{".into())));
            for p in pts {
                p3(g, 10, p);
            }
            g.push((305, V::S("}".into())));
        }
        g.push((303, V::S("}".into())));
    }
    g.push((301, V::S("}".into())));
    Ok(())
}

fn read_mline(o: &mut O, g: &mut Vec<(i32, V)>) -> R<()> {
    g.push((40, V::F(o.m.bd()?)));
    g.push((70, V::I(o.m.rc()? as i64)));
    p3(g, 10, o.m.bd3()?);
    ext(g, o.m.bd3()?);
    let oc = o.m.bs()?;
    g.push((71, V::I(if oc == 3 { 3 } else { 1 })));
    let nlines = o.m.rc()? as usize;
    let nv = o.m.bs()?.max(0) as usize;
    g.push((72, V::I(nv as i64)));
    g.push((73, V::I(nlines as i64)));
    for _ in 0..nv {
        p3(g, 11, o.m.bd3()?);
        p3(g, 12, o.m.bd3()?);
        p3(g, 13, o.m.bd3()?);
        for _ in 0..nlines {
            let np = o.m.bs()?.max(0);
            g.push((74, V::I(np as i64)));
            for _ in 0..np {
                g.push((41, V::F(o.m.bd()?)));
            }
            let nf = o.m.bs()?.max(0);
            g.push((75, V::I(nf as i64)));
            for _ in 0..nf {
                g.push((42, V::F(o.m.bd()?)));
            }
        }
    }
    Ok(())
}

fn read_dimension(o: &mut O, g: &mut Vec<(i32, V)>, name: &str) -> R<()> {
    let ver = o.ver;
    if ver >= Ver::R2010 {
        o.m.rc()?;
    }
    let n = o.m.bd3()?;
    let mid = o.m.rd2()?;
    let z = o.m.bd()?;
    let flags = o.m.rc()?;
    let text = o.tv()?;
    let trot = o.m.bd()?;
    let hdir = o.m.bd()?;
    o.m.bd3()?;
    o.m.bd()?;
    if ver >= Ver::R2000 {
        g.push((71, V::I(o.m.bs()? as i64)));
        o.m.bs()?;
        o.m.bd()?;
        o.m.bd()?;
    }
    if ver >= Ver::R2007 {
        o.m.bit()?;
        o.m.bit()?;
        o.m.bit()?;
    }
    let ins = o.m.rd2()?;
    let mut kind = match name {
        "DIM_LINEAR" => 0,
        "DIM_ALIGNED" => 1,
        "DIM_ANG2LN" => 2,
        "DIM_DIAMETER" => 3,
        "DIM_RADIUS" => 4,
        "DIM_ANG3PT" => 5,
        _ => 6,
    };
    match name {
        "DIM_LINEAR" | "DIM_ALIGNED" => {
            p3(g, 13, o.m.bd3()?);
            p3(g, 14, o.m.bd3()?);
            p3(g, 10, o.m.bd3()?);
            g.push((52, V::F(deg(o.m.bd()?))));
            if name == "DIM_LINEAR" {
                g.push((50, V::F(deg(o.m.bd()?))));
            }
        }
        "DIM_ANG3PT" => {
            p3(g, 10, o.m.bd3()?);
            p3(g, 13, o.m.bd3()?);
            p3(g, 14, o.m.bd3()?);
            p3(g, 15, o.m.bd3()?);
        }
        "DIM_ANG2LN" => {
            let a = o.m.rd2()?;
            p3(g, 16, [a[0], a[1], z]);
            p3(g, 13, o.m.bd3()?);
            p3(g, 14, o.m.bd3()?);
            p3(g, 15, o.m.bd3()?);
            p3(g, 10, o.m.bd3()?);
        }
        "DIM_RADIUS" => {
            p3(g, 10, o.m.bd3()?);
            p3(g, 15, o.m.bd3()?);
            g.push((40, V::F(o.m.bd()?)));
        }
        "DIM_DIAMETER" => {
            p3(g, 15, o.m.bd3()?);
            p3(g, 10, o.m.bd3()?);
            g.push((40, V::F(o.m.bd()?)));
        }
        _ => {
            p3(g, 10, o.m.bd3()?);
            p3(g, 13, o.m.bd3()?);
            p3(g, 14, o.m.bd3()?);
            if o.m.rc()? & 1 != 0 {
                kind |= 64;
            }
        }
    }
    if flags & 1 == 0 {
        kind |= 128;
    }
    if flags & 2 != 0 {
        kind |= 32;
    }
    p3(g, 11, [mid[0], mid[1], z]);
    p3(g, 12, [ins[0], ins[1], z]);
    g.push((70, V::I(kind)));
    g.push((1, V::S(text)));
    g.push((53, V::F(deg(trot))));
    g.push((51, V::F(deg(hdir))));
    ext(g, n);
    g.push((3, V::Name(o.hr()?)));
    g.push((2, V::Name(o.hr()?)));
    Ok(())
}
