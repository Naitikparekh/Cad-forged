//! The header variables section. The variables are stored in a fixed, version-dependent order with no names, so
//! reading one means reading everything before it; we read up to INSUNITS (R2000+) and keep what CadForge uses.
//! Ported from ACadSharp's `DwgHeaderReader` (MIT). A failure part-way keeps the values read so far.

use super::bits::{Bits, R};
use super::Ver;

#[derive(Debug, Clone)]
pub struct Header {
    pub ltscale: f64,
    pub textsize: f64,
    pub pdmode: i16,
    pub pdsize: f64,
    pub lunits: i16,
    pub luprec: i16,
    pub clayer: u64,
    pub dimscale: f64,
    pub dimasz: f64,
    pub dimexo: f64,
    pub dimexe: f64,
    pub dimtxt: f64,
    pub dimcen: f64,
    pub dimgap: f64,
    pub dimdec: Option<i16>,
    pub insunits: Option<i16>,
    /// The drawing's insertion base point.
    pub insbase: [f64; 3],
}

impl Default for Header {
    fn default() -> Self {
        Header { ltscale: 1.0, textsize: 2.5, pdmode: 0, pdsize: 0.0, lunits: 2, luprec: 4, clayer: 0, dimscale: 1.0, dimasz: 2.5, dimexo: 0.625, dimexe: 1.25, dimtxt: 2.5, dimcen: 2.5, dimgap: 0.625, dimdec: None, insunits: None, insbase: [0.0; 3] }
    }
}

struct HR<'a> {
    m: Bits<'a>,
    t: Option<Bits<'a>>,
    h: Option<Bits<'a>>,
}

impl HR<'_> {
    fn tv(&mut self) -> R<String> {
        match self.t.as_mut() {
            Some(t) => t.tv(),
            None => self.m.tv(),
        }
    }
    fn h(&mut self) -> R<u64> {
        match self.h.as_mut() {
            Some(h) => h.handle(0),
            None => self.m.handle(0),
        }
    }
    fn cmc(&mut self) -> R<()> {
        let ver = self.m.ver;
        self.m.bs()?;
        if ver >= Ver::R2004 {
            self.m.bl()?;
            let flags = self.m.rc()?;
            if flags & 1 != 0 {
                self.tv()?;
            }
            if flags & 2 != 0 {
                self.tv()?;
            }
        }
        Ok(())
    }
}

pub fn read(d: &[u8], ver: Ver, maint: u8) -> Header {
    let mut out = Header::default();
    let _ = read_into(d, ver, maint, &mut out);
    out
}

fn read_into(d: &[u8], ver: Ver, maint: u8, o: &mut Header) -> R<()> {
    let r13_14 = ver <= Ver::R14;
    let r2000 = ver >= Ver::R2000;
    let r2004 = ver >= Ver::R2004;
    let r2007 = ver >= Ver::R2007;
    let r2010 = ver >= Ver::R2010;
    let r2013 = ver >= Ver::R2013;
    let mut m = Bits::new(d, ver);
    m.skip_bytes(16)?; // sentinel
    m.rl()?; // size
    if (r2010 && maint > 3) || ver >= Ver::R2018 {
        m.rl()?;
    }
    let initial = m.pos as i64;
    let mut r = if r2007 {
        let size_bits = m.rl()? as i64;
        let last = initial + size_bits - 1;
        let mut t = Bits::new(d, ver);
        let has = t.set_string_stream(last)?;
        let h = Bits::at(d, ver, (last + 1).max(0) as usize);
        HR { m, t: if has { Some(t) } else { None }, h: Some(h) }
    } else {
        HR { m, t: None, h: None }
    };
    if r2013 {
        r.m.bll()?;
    }
    for _ in 0..4 {
        r.m.bd()?;
    }
    for _ in 0..4 {
        r.tv()?;
    }
    r.m.bl()?;
    r.m.bl()?;
    if r13_14 {
        r.m.bs()?;
    }
    if !r2004 {
        r.h()?;
    }
    r.m.bit()?; // DIMASO
    r.m.bit()?; // DIMSHO
    if r13_14 {
        r.m.bit()?;
    }
    for _ in 0..7 {
        r.m.bit()?; // PLINEGEN ORTHOMODE REGENMODE FILLMODE QTEXTMODE PSLTSCALE LIMCHECK
    }
    if r13_14 {
        r.m.bit()?;
    }
    if r2004 {
        r.m.bit()?;
    }
    for _ in 0..4 {
        r.m.bit()?; // USRTIMER SKPOLY ANGDIR SPLFRAME
    }
    if r13_14 {
        r.m.bit()?;
        r.m.bit()?;
    }
    r.m.bit()?; // MIRRTEXT
    r.m.bit()?; // WORLDVIEW
    if r13_14 {
        r.m.bit()?;
    }
    for _ in 0..3 {
        r.m.bit()?; // TILEMODE PLIMCHECK VISRETAIN
    }
    if r13_14 {
        r.m.bit()?;
    }
    r.m.bit()?; // DISPSILH
    r.m.bit()?; // PELLIPSE
    r.m.bs()?; // PROXYGRAPHICS
    if r13_14 {
        r.m.bs()?;
    }
    r.m.bs()?; // TREEDEPTH
    o.lunits = r.m.bs()?;
    o.luprec = r.m.bs()?;
    r.m.bs()?; // AUNITS
    r.m.bs()?; // AUPREC
    if r13_14 {
        r.m.bs()?;
    }
    r.m.bs()?; // ATTMODE
    if r13_14 {
        r.m.bs()?;
    }
    o.pdmode = r.m.bs()?;
    if r13_14 {
        r.m.bs()?;
    }
    if r2004 {
        for _ in 0..3 {
            r.m.bl()?;
        }
    }
    for _ in 0..16 {
        r.m.bs()?; // USERI1-5 SPLINESEGS SURFU SURFV SURFTYPE SURFTAB1 SURFTAB2 SPLINETYPE SHADEDGE SHADEDIF UNITMODE MAXACTVP
    }
    r.m.bs()?; // ISOLINES
    r.m.bs()?; // CMLJUST
    r.m.bs()?; // TEXTQLTY
    o.ltscale = r.m.bd()?;
    o.textsize = r.m.bd()?;
    for _ in 0..5 {
        r.m.bd()?; // TRACEWID SKETCHINC FILLETRAD THICKNESS ANGBASE
    }
    o.pdsize = r.m.bd()?;
    for _ in 0..13 {
        r.m.bd()?; // PLINEWID USERR1-5 CHAMFERA-D FACETRES CMLSCALE CELTSCALE
    }
    r.tv()?; // MENUNAME
    for _ in 0..4 {
        r.m.bl()?; // TDCREATE TDUPDATE
    }
    if r2004 {
        for _ in 0..3 {
            r.m.bl()?;
        }
    }
    for _ in 0..4 {
        r.m.bl()?; // TDINDWG TDUSRTIMER
    }
    r.cmc()?; // CECOLOR
    r.m.handle(0)?; // HANDSEED (main stream)
    o.clayer = r.h()?;
    r.h()?; // TEXTSTYLE
    r.h()?; // CELTYPE
    if r2007 {
        r.h()?;
    }
    r.h()?; // DIMSTYLE
    r.h()?; // CMLSTYLE
    if r2000 {
        r.m.bd()?;
    }
    r.m.bd3()?;
    r.m.bd3()?;
    r.m.bd3()?;
    r.m.rd2()?;
    r.m.rd2()?;
    r.m.bd()?;
    r.m.bd3()?;
    r.m.bd3()?;
    r.m.bd3()?;
    r.h()?;
    if r2000 {
        r.h()?;
        r.m.bs()?;
        r.h()?;
        for _ in 0..6 {
            r.m.bd3()?;
        }
    }
    // model space
    o.insbase = r.m.bd3()?;
    r.m.bd3()?;
    r.m.bd3()?;
    r.m.rd2()?;
    r.m.rd2()?;
    r.m.bd()?;
    r.m.bd3()?;
    r.m.bd3()?;
    r.m.bd3()?;
    r.h()?;
    if r2000 {
        r.h()?;
        r.m.bs()?;
        r.h()?;
        for _ in 0..6 {
            r.m.bd3()?;
        }
        r.tv()?; // DIMPOST
        r.tv()?; // DIMAPOST
    }
    if r13_14 {
        for _ in 0..11 {
            r.m.bit()?;
        }
        r.m.rc()?;
        r.m.rc()?;
        r.m.bit()?;
        r.m.bit()?;
        r.m.rc()?;
        r.m.rc()?;
        r.m.rc()?;
        r.m.bit()?;
        r.m.rc()?;
        r.m.rc()?;
        r.m.rc()?;
        r.m.rc()?;
        r.m.bs()?;
        r.m.bs()?;
        o.dimdec = Some(r.m.bs()?);
        r.m.bs()?;
        r.m.bs()?;
        r.m.bs()?;
        r.h()?;
    }
    o.dimscale = r.m.bd()?;
    o.dimasz = r.m.bd()?;
    o.dimexo = r.m.bd()?;
    r.m.bd()?; // DIMDLI
    o.dimexe = r.m.bd()?;
    for _ in 0..4 {
        r.m.bd()?; // DIMRND DIMDLE DIMTP DIMTM
    }
    if r2007 {
        r.m.bd()?;
        r.m.bd()?;
        r.m.bs()?;
        r.cmc()?;
    }
    if r2000 {
        for _ in 0..6 {
            r.m.bit()?;
        }
        r.m.bs()?;
        r.m.bs()?;
        r.m.bs()?;
    }
    if r2007 {
        r.m.bs()?;
    }
    o.dimtxt = r.m.bd()?;
    o.dimcen = r.m.bd()?;
    for _ in 0..5 {
        r.m.bd()?; // DIMTSZ DIMALTF DIMLFAC DIMTVP DIMTFAC
    }
    o.dimgap = r.m.bd()?;
    if r13_14 {
        for _ in 0..5 {
            r.tv()?;
        }
    }
    if r2000 {
        r.m.bd()?;
        r.m.bit()?;
        r.m.bs()?;
        for _ in 0..4 {
            r.m.bit()?;
        }
    }
    r.cmc()?;
    r.cmc()?;
    r.cmc()?;
    if !r2000 {
        return Ok(());
    }
    r.m.bs()?; // DIMADEC
    o.dimdec = Some(r.m.bs()?);
    for _ in 0..9 {
        r.m.bs()?; // DIMTDEC DIMALTU DIMALTTD DIMAUNIT DIMFRAC DIMLUNIT DIMDSEP DIMTMOVE DIMJUST
    }
    r.m.bit()?;
    r.m.bit()?;
    for _ in 0..4 {
        r.m.bs()?; // DIMTOLJ DIMTZIN DIMALTZ DIMALTTZ
    }
    r.m.bit()?; // DIMUPT
    r.m.bs()?; // DIMATFIT
    if r2007 {
        r.m.bit()?;
    }
    if r2010 {
        r.m.bit()?;
        r.m.bd()?;
        r.tv()?;
        r.m.bd()?;
        r.tv()?;
    }
    for _ in 0..5 {
        r.h()?; // DIMTXSTY DIMLDRBLK DIMBLK DIMBLK1 DIMBLK2
    }
    if r2007 {
        for _ in 0..3 {
            r.h()?;
        }
    }
    r.m.bs()?; // DIMLWD
    r.m.bs()?; // DIMLWE
    for _ in 0..9 {
        r.h()?; // control objects
    }
    if ver <= Ver::R2000 {
        r.h()?;
    }
    for _ in 0..3 {
        r.h()?; // ACAD_GROUP ACAD_MLINESTYLE named objects
    }
    r.m.bs()?; // TSTACKALIGN
    r.m.bs()?; // TSTACKSIZE
    r.tv()?; // HYPERLINKBASE
    r.tv()?; // STYLESHEET
    for _ in 0..3 {
        r.h()?; // LAYOUTS PLOTSETTINGS PLOTSTYLES
    }
    if r2004 {
        r.h()?;
        r.h()?;
    }
    if r2007 {
        r.h()?;
        if r2013 {
            r.h()?;
        }
    }
    r.m.bl()?; // flags
    o.insunits = Some(r.m.bs()?);
    Ok(())
}
