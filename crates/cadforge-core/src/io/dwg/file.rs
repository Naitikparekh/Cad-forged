//! The file-level structure: which sections exist and where. R13-R2000 store sections uncompressed at offsets given in
//! the file header; R2004, R2010 and later split them into compressed pages listed in an encrypted header; R2007 adds
//! an interleaved page encoding and its own compression. Ported from ACadSharp's `DwgReader` (MIT).

use super::bits::{Bits, R};
use super::lz77::{decompress18, decompress21};
use super::Ver;
use std::collections::HashMap;

const BAD: &str = "This DWG file is damaged (its section table cannot be read).";

pub struct DwgFile<'a> {
    pub ver: Ver,
    pub maint: u8,
    pub data: &'a [u8],
    /// R2004+: decompressed sections by name ("AcDb:Header", ...).
    pub sections: HashMap<String, Vec<u8>>,
    /// R13-R2000: section number -> (offset, size).
    pub records: HashMap<u8, (usize, usize)>,
}

fn rl(d: &[u8], at: usize) -> R<u32> {
    let b = d.get(at..at + 4).ok_or(BAD)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn rll(d: &[u8], at: usize) -> R<u64> {
    let b = d.get(at..at + 8).ok_or(BAD)?;
    let mut a = [0u8; 8];
    a.copy_from_slice(b);
    Ok(u64::from_le_bytes(a))
}

pub fn version_of(d: &[u8]) -> Option<Ver> {
    let v = d.get(0..6)?;
    Some(match v {
        b"AC1012" => Ver::R13,
        b"AC1014" => Ver::R14,
        b"AC1015" => Ver::R2000,
        b"AC1018" => Ver::R2004,
        b"AC1021" => Ver::R2007,
        b"AC1024" => Ver::R2010,
        b"AC1027" => Ver::R2013,
        b"AC1032" => Ver::R2018,
        _ => return None,
    })
}

impl<'a> DwgFile<'a> {
    pub fn open(data: &'a [u8]) -> R<DwgFile<'a>> {
        let ver = match version_of(data) {
            Some(v) => v,
            None => {
                let tag = String::from_utf8_lossy(data.get(0..6).unwrap_or(b"")).to_string();
                return Err(if tag.starts_with("AC") {
                    format!("DWG version {tag} is not supported (R13 to 2018 are: AC1012 to AC1032).")
                } else {
                    "This is not a DWG file.".into()
                });
            }
        };
        let mut f = DwgFile { ver, maint: 0, data, sections: HashMap::new(), records: HashMap::new() };
        match ver {
            Ver::R13 | Ver::R14 | Ver::R2000 => f.header15()?,
            Ver::R2007 => f.header21()?,
            _ => f.header18()?,
        }
        Ok(f)
    }

    /// The bytes of a named section (R13-R2000: from its offset to the end of the file).
    pub fn section(&self, name: &str) -> Option<&[u8]> {
        if self.ver >= Ver::R2004 {
            return self.sections.get(name).map(|v| v.as_slice());
        }
        let n = match name {
            "AcDb:Header" => 0,
            "AcDb:Classes" => 1,
            "AcDb:Handles" => 2,
            _ => return None,
        };
        let (at, _) = *self.records.get(&n)?;
        self.data.get(at..)
    }

    /// The bytes object offsets refer to (R13-R2000: the whole file).
    pub fn objects(&self) -> Option<&[u8]> {
        if self.ver >= Ver::R2004 {
            self.sections.get("AcDb:AcDbObjects").map(|v| v.as_slice())
        } else {
            Some(self.data)
        }
    }

    fn header15(&mut self) -> R<()> {
        self.maint = *self.data.get(0x0B).unwrap_or(&0);
        let n = rl(self.data, 0x15)? as usize;
        if n > 64 {
            return Err(BAD.into());
        }
        for k in 0..n {
            let at = 0x19 + k * 9;
            let num = *self.data.get(at).ok_or(BAD)?;
            let seek = rl(self.data, at + 1)? as usize;
            let size = rl(self.data, at + 5)? as usize;
            self.records.insert(num, (seek, size));
        }
        Ok(())
    }

    fn header18(&mut self) -> R<()> {
        let d = self.data;
        self.maint = *d.get(0x0B).unwrap_or(&0);
        // 0x80: 0x6C bytes encrypted with a fixed pseudo-random sequence
        let enc = d.get(0x80..0x80 + 0x6C).ok_or(BAD)?;
        let mut h = enc.to_vec();
        let mut seed: u32 = 1;
        for b in h.iter_mut() {
            seed = seed.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
            *b ^= (seed >> 16) as u8;
        }
        if &h[0..11] != b"AcFssFcAJMB" {
            return Err("This DWG file's header does not decrypt: the file is damaged or not a DWG.".into());
        }
        let section_map_id = rl(&h, 0x5C)?;
        let page_map_addr = rll(&h, 0x54)? as usize + 0x100;

        // page map: (page number, size) pairs, positions accumulate from 0x100
        let page_map = system_page(d, page_map_addr)?;
        let mut pages: HashMap<i32, usize> = HashMap::new();
        let mut i = 0;
        let mut total = 0x100usize;
        while i + 8 <= page_map.len() {
            let num = rl(&page_map, i)? as i32;
            let size = rl(&page_map, i + 4)? as usize;
            i += 8;
            if num >= 0 {
                pages.insert(num, total);
            } else {
                i += 16;
            }
            total = total.checked_add(size).ok_or(BAD)?;
        }

        // section map
        let at = *pages.get(&(section_map_id as i32)).ok_or(BAD)?;
        let smap = system_page(d, at)?;
        let ndesc = rl(&smap, 0)? as usize;
        let mut p = 20;
        for _ in 0..ndesc.min(1000) {
            let size = rll(&smap, p)?;
            let page_count = rl(&smap, p + 8)? as usize;
            let max_size = rl(&smap, p + 12)? as u64;
            let compressed = rl(&smap, p + 20)? == 2;
            let raw_name = smap.get(p + 32..p + 96).ok_or(BAD)?;
            let name: String = raw_name.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect();
            p += 96;
            let mut out: Vec<u8> = Vec::new();
            for _ in 0..page_count.min(100_000) {
                let num = rl(&smap, p)? as i32;
                let csize = rl(&smap, p + 4)? as usize;
                let offset = rll(&smap, p + 8)?;
                p += 16;
                // zero pages are not stored: fill the gap
                while (out.len() as u64) < offset && max_size > 0 && out.len() < (1 << 31) {
                    out.resize(out.len() + max_size as usize, 0);
                }
                let Some(&seek) = pages.get(&num) else { continue };
                let data_at = seek + 32;
                let raw = d.get(data_at..data_at.checked_add(csize).ok_or(BAD)?).ok_or(BAD)?;
                if compressed {
                    let page = decompress18(raw, max_size.max(0x7400) as usize * 2)?;
                    out.extend_from_slice(&page);
                } else {
                    out.extend_from_slice(raw);
                }
            }
            if size > 0 && (out.len() as u64) > size {
                out.truncate(size as usize);
            }
            if !name.is_empty() {
                self.sections.insert(name, out);
            }
        }
        Ok(())
    }

    fn header21(&mut self) -> R<()> {
        let d = self.data;
        self.maint = *d.get(0x0B).unwrap_or(&0);
        let enc = d.get(0x80..0x80 + 0x400).ok_or(BAD)?;
        let dec = deinterleave(enc, 3 * 239, 3, 239);
        let comp_len = i32::from_le_bytes([dec[24], dec[25], dec[26], dec[27]]);
        if comp_len <= 0 {
            return Err("Uncompressed R2007 headers are not supported.".into());
        }
        let meta = decompress21(&dec, 32, comp_len as usize, 0x110)?;
        let m = |k: usize| rll(&meta, k * 8);
        let (pages_map_offset, pages_map_size_c, pages_map_size_u, pages_map_cf) = (m(7)?, m(10)?, m(11)?, m(3)?);
        let (sections_map_size_c, sections_map_id, sections_map_size_u, sections_map_cf) = (m(22)?, m(24)?, m(25)?, m(27)?);

        let pmap = page_buffer(d, pages_map_offset, pages_map_size_c, pages_map_size_u, pages_map_cf, 0xEF)?;
        let mut pages: HashMap<i64, (u64, u64)> = HashMap::new();
        let mut off = 0u64;
        let mut i = 0;
        while i + 16 <= pmap.len() {
            let size = rll(&pmap, i)?;
            let id = (rll(&pmap, i + 8)? as i64).abs();
            pages.insert(id, (off, size));
            off = off.wrapping_add(size);
            i += 16;
        }
        let (smap_at, _) = *pages.get(&(sections_map_id as i64)).ok_or(BAD)?;
        let smap = page_buffer(d, smap_at, sections_map_size_c, sections_map_size_u, sections_map_cf, 239)?;
        let mut p = 0;
        while p + 64 <= smap.len() {
            let max_size = rll(&smap, p + 8)?;
            let name_len = rll(&smap, p + 32)? as usize;
            let encoding = rll(&smap, p + 48)?;
            let page_count = rll(&smap, p + 56)? as usize;
            p += 64;
            let mut name = String::new();
            if name_len > 0 {
                let raw = smap.get(p..p + name_len).ok_or(BAD)?;
                let units: Vec<u16> = raw.chunks(2).filter(|c| c.len() == 2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                name = String::from_utf16_lossy(&units).replace('\0', "");
                p += name_len;
            }
            let mut out: Vec<u8> = Vec::new();
            for _ in 0..page_count.min(100_000) {
                let poff = rll(&smap, p)?;
                let page_id = rll(&smap, p + 16)? as i64;
                let usize_ = rll(&smap, p + 24)? as usize;
                let csize = rll(&smap, p + 32)? as usize;
                p += 56;
                if (out.len() as u64) < poff && poff - (out.len() as u64) < (1 << 31) {
                    out.resize(poff as usize, 0);
                }
                let Some(&(seek, psize)) = pages.get(&page_id) else { continue };
                let at = (seek as usize).checked_add(0x480).ok_or(BAD)?;
                let mut bytes = d.get(at..at.checked_add(psize as usize).ok_or(BAD)?).ok_or(BAD)?.to_vec();
                if encoding == 4 {
                    let v = (csize + 7) & !7;
                    let factor = v.div_ceil(251);
                    bytes = deinterleave(&bytes, factor * 251, factor, 251);
                }
                if csize != usize_ {
                    bytes = decompress21(&bytes, 0, csize, usize_)?;
                }
                bytes.truncate(usize_);
                out.extend_from_slice(&bytes);
            }
            let _ = max_size;
            if name_len > 0 && !name.is_empty() {
                self.sections.insert(name, out);
            }
        }
        Ok(())
    }

    /// The object map: handle -> offset of the object (in [`objects`](Self::objects)).
    pub fn handle_map(&self) -> R<Vec<(u64, usize)>> {
        let d = self.section("AcDb:Handles").ok_or("The DWG file has no object map.")?;
        let mut b = Bits::new(d, self.ver);
        let mut out = vec![];
        loop {
            let hi = b.rc()? as usize;
            let lo = b.rc()? as usize;
            let size = (hi << 8) | lo;
            if size <= 2 {
                break;
            }
            let start = b.pos / 8;
            let end = start + (size - 2).min(2032);
            let (mut handle, mut loc) = (0u64, 0i64);
            while b.pos / 8 < end {
                let dh = b.mc()?;
                let dl = b.smc()?;
                handle = handle.wrapping_add(dh);
                loc = loc.wrapping_add(dl);
                if dh > 0 && loc >= 0 {
                    out.push((handle, loc as usize));
                }
            }
            b.pos = end * 8;
            b.skip_bytes(2)?; // CRC
        }
        Ok(out)
    }

    /// Class number -> (DXF name, whether its objects are entities).
    pub fn classes(&self) -> HashMap<u16, (String, bool)> {
        self.read_classes().unwrap_or_default()
    }

    fn read_classes(&self) -> R<HashMap<u16, (String, bool)>> {
        let mut out = HashMap::new();
        let Some(d) = self.section("AcDb:Classes") else { return Ok(out) };
        let mut b = Bits::new(d, self.ver);
        b.skip_bytes(16)?; // sentinel
        let size = b.rl()? as usize;
        let mut end = b.pos / 8 + size;
        if (self.ver >= Ver::R2010 && self.maint > 3) || self.ver > Ver::R2013 {
            b.rl()?;
        }
        let mut text: Option<Bits> = None;
        let mut end_bits = end * 8;
        if self.ver >= Ver::R2007 {
            let flag_pos = b.pos as i64 + b.rl()? as i64 - 1;
            let mut t = Bits::new(d, self.ver);
            if t.set_string_stream(flag_pos)? {
                end_bits = t.pos;
                text = Some(t);
            } else {
                end_bits = flag_pos as usize;
            }
            end = end_bits.div_ceil(8);
            b.bl()?;
            b.bit()?;
        }
        if self.ver == Ver::R2004 {
            b.bs()?;
            b.rc()?;
            b.rc()?;
            b.bit()?;
        }
        let _ = end;
        while b.pos < end_bits {
            let r = (|| -> R<(u16, (String, bool))> {
                let num = b.bs()? as u16;
                b.bs()?;
                let mut tv = |b: &mut Bits| -> R<String> {
                    match text.as_mut() {
                        Some(t) => t.tv(),
                        None => b.tv(),
                    }
                };
                tv(&mut b)?;
                tv(&mut b)?;
                let dxf = tv(&mut b)?;
                b.bit()?;
                let item = b.bs()?;
                if self.ver >= Ver::R2004 {
                    for _ in 0..5 {
                        b.bl()?;
                    }
                }
                Ok((num, (dxf, item == 0x1F2)))
            })();
            match r {
                Ok((num, dxf)) => {
                    out.insert(num, dxf);
                }
                Err(_) => break,
            }
        }
        Ok(out)
    }
}

/// A system page (page map, section map) of R2004+: a 20 byte header then compressed data.
fn system_page(d: &[u8], at: usize) -> R<Vec<u8>> {
    let usize_ = rl(d, at + 4)? as usize;
    let csize = rl(d, at + 8)? as usize;
    let raw = d.get(at + 20..(at + 20).checked_add(csize).ok_or(BAD)?).ok_or(BAD)?;
    let mut out = decompress18(raw, usize_.max(16) * 2)?;
    out.truncate(usize_);
    Ok(out)
}

/// R2007 pages are stored interleaved: byte `k` of block `i` sits at `i + k * factor`.
fn deinterleave(enc: &[u8], out_len: usize, factor: usize, block: usize) -> Vec<u8> {
    let mut out = vec![0u8; out_len];
    let (mut idx, mut left) = (0usize, out_len);
    for i in 0..factor {
        if i >= enc.len() {
            break;
        }
        let size = left.min(block);
        left -= size;
        let mut c = i;
        for _ in 0..size {
            out[idx] = enc.get(c).copied().unwrap_or(0);
            idx += 1;
            c += factor;
        }
    }
    out
}

/// An R2007 system page: interleaved, then compressed.
fn page_buffer(d: &[u8], offset: u64, comp: u64, uncomp: u64, cf: u64, block: usize) -> R<Vec<u8>> {
    let v = (comp + 7) & !7;
    let total = v.checked_mul(cf).ok_or(BAD)? as usize;
    if total > (1 << 30) || uncomp > (1 << 30) {
        return Err(BAD.into());
    }
    let factor = total.div_ceil(block);
    let len = factor * 255;
    let at = (offset as usize).checked_add(0x480).ok_or(BAD)?;
    let raw = d.get(at..at.checked_add(len).ok_or(BAD)?.min(d.len())).ok_or(BAD)?;
    let data = deinterleave(raw, total, factor, block);
    decompress21(&data, 0, comp as usize, uncomp as usize)
}
