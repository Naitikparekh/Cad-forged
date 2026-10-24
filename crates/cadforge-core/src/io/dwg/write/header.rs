//! The header variables section of an AutoCAD 2000 file: every variable in its fixed order. Values CadForge keeps
//! come from the drawing's DXF header, the rest are AutoCAD's defaults. Field order ported from ACadSharp (MIT).

use super::bitw::{crc16, BitW};
use super::dxfin::Rec;
use std::collections::HashMap;

/// Handles the header refers to.
pub struct Refs {
    pub handseed: u64,
    pub clayer: u64,
    pub textstyle: u64,
    pub celtype: u64,
    pub dimstyle: u64,
    pub cmlstyle: u64,
    pub dimtxsty: u64,
    /// BLOCK, LAYER, STYLE, LTYPE, VIEW, UCS, VPORT, APPID, DIMSTYLE and VP_ENT_HDR control objects.
    pub controls: [u64; 10],
    pub dict_group: u64,
    pub dict_mline: u64,
    pub dict_root: u64,
    pub dict_layout: u64,
    pub dict_plotsettings: u64,
    pub dict_plotstyle: u64,
    pub paper: u64,
    pub model: u64,
    pub bylayer: u64,
    pub byblock: u64,
    pub continuous: u64,
}

pub const START: [u8; 16] = [0xCF, 0x7B, 0x1F, 0x23, 0xFD, 0xDE, 0x38, 0xA9, 0x5F, 0x7C, 0x68, 0xB8, 0x4E, 0x6D, 0x33, 0x5F];
pub const END: [u8; 16] = [0x30, 0x84, 0xE0, 0xDC, 0x02, 0x21, 0xC7, 0x56, 0xA0, 0x83, 0x97, 0x47, 0xB1, 0x92, 0xCC, 0xA0];

/// The section, sentinels and CRC included. `julian` is the creation / update date (day, milliseconds).
pub fn write(h: &HashMap<String, Rec>, r: &Refs, julian: (i32, i32)) -> Vec<u8> {
    let f = |name: &str, code: i32, d: f64| h.get(name).map_or(d, |v| v.fd(code, d));
    let i = |name: &str, d: i64| h.get(name).map_or(d, |v| v.i(70, d)) as i16;
    let p3 = |name: &str, d: [f64; 3]| h.get(name).map_or(d, |v| [v.fd(10, d[0]), v.fd(20, d[1]), v.fd(30, d[2])]);
    let p2 = |name: &str, d: [f64; 2]| h.get(name).map_or(d, |v| [v.fd(10, d[0]), v.fd(20, d[1])]);
    let metric = i("$MEASUREMENT", 1) == 1;
    let mut m = BitW::new();
    for v in [412148564080.0, 1.0, 1.0, 1.0] {
        m.bd(v);
    }
    m.tv("m");
    for _ in 0..3 {
        m.tv("");
    }
    m.bl(24);
    m.bl(0);
    m.h(5, 0); // current viewport entity header
    m.bit(true); // DIMASO
    m.bit(true); // DIMSHO
    for b in [false, false, true, true, false, true, false] {
        m.bit(b); // PLINEGEN ORTHOMODE REGENMODE FILLMODE QTEXTMODE PSLTSCALE LIMCHECK
    }
    for b in [true, false, false, false] {
        m.bit(b); // USRTIMER SKPOLY ANGDIR SPLFRAME
    }
    m.bit(false); // MIRRTEXT
    m.bit(true); // WORLDVIEW
    for b in [true, false, true] {
        m.bit(b); // TILEMODE PLIMCHECK VISRETAIN
    }
    m.bit(false); // DISPSILH
    m.bit(false); // PELLIPSE
    m.bs(1); // PROXYGRAPHICS
    m.bs(3020); // TREEDEPTH
    m.bs(i("$LUNITS", 2));
    m.bs(i("$LUPREC", 4));
    m.bs(0); // AUNITS
    m.bs(i("$AUPREC", 0));
    m.bs(1); // ATTMODE
    m.bs(i("$PDMODE", 0));
    for v in [0, 0, 0, 0, 0, 8, 6, 6, 6, 6, 6, 6, 3, 70, 0, 64] {
        m.bs(v); // USERI1-5 SPLINESEGS SURFU SURFV SURFTYPE SURFTAB1 SURFTAB2 SPLINETYPE SHADEDGE SHADEDIF UNITMODE MAXACTVP
    }
    m.bs(4); // ISOLINES
    m.bs(0); // CMLJUST
    m.bs(50); // TEXTQLTY
    m.bd(f("$LTSCALE", 40, 1.0));
    m.bd(f("$TEXTSIZE", 40, 2.5));
    m.bd(if metric { 1.0 } else { 0.05 }); // TRACEWID
    m.bd(if metric { 1.0 } else { 0.1 }); // SKETCHINC
    m.bd(0.0); // FILLETRAD
    m.bd(0.0); // THICKNESS
    m.bd(0.0); // ANGBASE
    m.bd(f("$PDSIZE", 40, 0.0));
    m.bd(0.0); // PLINEWID
    for _ in 0..5 {
        m.bd(0.0); // USERR1-5
    }
    for _ in 0..4 {
        m.bd(0.0); // CHAMFERA-D
    }
    m.bd(0.5); // FACETRES
    m.bd(if metric { 20.0 } else { 1.0 }); // CMLSCALE
    m.bd(1.0); // CELTSCALE
    m.tv("acad"); // MENUNAME
    for _ in 0..2 {
        m.bl(julian.0); // TDCREATE, TDUPDATE
        m.bl(julian.1);
    }
    for _ in 0..4 {
        m.bl(0); // TDINDWG TDUSRTIMER
    }
    m.bs(256); // CECOLOR
    m.h(0, r.handseed);
    m.h(5, r.clayer);
    m.h(5, r.textstyle);
    m.h(5, r.celtype);
    m.h(5, r.dimstyle);
    m.h(5, r.cmlstyle);
    m.bd(0.0); // PSVPSCALE
    // paper space
    m.bd3([0.0; 3]); // INSBASE
    m.bd3([1e20, 1e20, 1e20]); // EXTMIN
    m.bd3([-1e20, -1e20, -1e20]); // EXTMAX
    m.rd2([0.0, 0.0]); // LIMMIN
    m.rd2(if metric { [420.0, 297.0] } else { [12.0, 9.0] }); // LIMMAX
    ucs_block(&mut m);
    // model space
    m.bd3(p3("$INSBASE", [0.0; 3]));
    m.bd3(p3("$EXTMIN", [0.0; 3]));
    m.bd3(p3("$EXTMAX", [0.0; 3]));
    m.rd2(p2("$LIMMIN", [0.0, 0.0]));
    m.rd2(p2("$LIMMAX", if metric { [420.0, 297.0] } else { [12.0, 9.0] }));
    ucs_block(&mut m);
    m.tv(""); // DIMPOST
    m.tv(""); // DIMAPOST
    m.bd(f("$DIMSCALE", 40, 1.0));
    m.bd(f("$DIMASZ", 40, 2.5));
    m.bd(f("$DIMEXO", 40, 0.625));
    m.bd(if metric { 3.75 } else { 0.38 }); // DIMDLI
    m.bd(f("$DIMEXE", 40, 1.25));
    for _ in 0..4 {
        m.bd(0.0); // DIMRND DIMDLE DIMTP DIMTM
    }
    for b in [false, false, !metric, !metric, false, false] {
        m.bit(b); // DIMTOL DIMLIM DIMTIH DIMTOH DIMSE1 DIMSE2
    }
    m.bs(if metric { 1 } else { 0 }); // DIMTAD
    m.bs(if metric { 8 } else { 0 }); // DIMZIN
    m.bs(0); // DIMAZIN
    m.bd(f("$DIMTXT", 40, 2.5));
    m.bd(f("$DIMCEN", 40, 2.5));
    m.bd(0.0); // DIMTSZ
    m.bd(if metric { 0.03937007874 } else { 25.4 }); // DIMALTF
    m.bd(1.0); // DIMLFAC
    m.bd(0.0); // DIMTVP
    m.bd(1.0); // DIMTFAC
    m.bd(f("$DIMGAP", 40, 0.625));
    m.bd(0.0); // DIMALTRND
    m.bit(false); // DIMALT
    m.bs(if metric { 3 } else { 2 }); // DIMALTD
    m.bit(metric); // DIMTOFL
    m.bit(false); // DIMSAH
    m.bit(false); // DIMTIX
    m.bit(false); // DIMSOXD
    for _ in 0..3 {
        m.bs(0); // DIMCLRD DIMCLRE DIMCLRT
    }
    m.bs(0); // DIMADEC
    m.bs(i("$DIMDEC", if metric { 2 } else { 4 }));
    m.bs(if metric { 2 } else { 4 }); // DIMTDEC
    m.bs(2); // DIMALTU
    m.bs(2); // DIMALTTD
    m.bs(0); // DIMAUNIT
    m.bs(0); // DIMFRAC
    m.bs(2); // DIMLUNIT
    m.bs(b'.' as i16); // DIMDSEP (CadForge writes dimension text with a point)
    m.bs(0); // DIMTMOVE
    m.bs(0); // DIMJUST
    m.bit(false); // DIMSD1
    m.bit(false); // DIMSD2
    m.bs(if metric { 0 } else { 1 }); // DIMTOLJ
    m.bs(if metric { 8 } else { 0 }); // DIMTZIN
    m.bs(0); // DIMALTZ
    m.bs(0); // DIMALTTZ
    m.bit(false); // DIMUPT
    m.bs(3); // DIMATFIT
    m.h(5, r.dimtxsty);
    for _ in 0..4 {
        m.h(5, 0); // DIMLDRBLK DIMBLK DIMBLK1 DIMBLK2
    }
    m.bs(-2); // DIMLWD
    m.bs(-2); // DIMLWE
    for c in r.controls {
        m.h(3, c);
    }
    m.h(5, r.dict_group);
    m.h(5, r.dict_mline);
    m.h(3, r.dict_root);
    m.bs(1); // TSTACKALIGN
    m.bs(70); // TSTACKSIZE
    m.tv(""); // HYPERLINKBASE
    m.tv(""); // STYLESHEET
    m.h(5, r.dict_layout);
    m.h(5, r.dict_plotsettings);
    m.h(5, r.dict_plotstyle);
    // CELWEIGHT ByLayer, line weights not displayed, long names, colour-dependent plot styles
    m.bl(29 | 0x200 | 0x800 | 0x2000);
    m.bs(i("$INSUNITS", 4));
    m.bs(0); // CEPSNTYPE
    m.tv("{00000000-0000-0000-0000-000000000000}"); // FINGERPRINTGUID
    m.tv("{00000000-0000-0000-0000-000000000000}"); // VERSIONGUID
    m.h(5, r.paper);
    m.h(5, r.model);
    m.h(5, r.bylayer);
    m.h(5, r.byblock);
    m.h(5, r.continuous);
    for _ in 0..4 {
        m.bs(-1);
    }
    m.align();
    let mut out = START.to_vec();
    let mut body = (m.buf.len() as i32).to_le_bytes().to_vec();
    body.extend_from_slice(&m.buf);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc16(0xC0C1, &body).to_le_bytes());
    out.extend_from_slice(&END);
    out
}

/// ELEVATION, UCSORG, UCSXDIR, UCSYDIR, UCSNAME, UCSORTHOREF, UCSORTHOVIEW, UCSBASE and the six orthographic origins.
fn ucs_block(m: &mut BitW) {
    m.bd(0.0);
    m.bd3([0.0, 0.0, 0.0]);
    m.bd3([1.0, 0.0, 0.0]);
    m.bd3([0.0, 1.0, 0.0]);
    m.h(5, 0);
    m.h(5, 0);
    m.bs(0);
    m.h(5, 0);
    for _ in 0..6 {
        m.bd3([0.0; 3]);
    }
}
