//! The AutoCAD 2004 (AC1018) file container: sections cut into pages of at most 0x7400 bytes, each LZ77-compressed
//! (the small information sections stored as they are), a section map and a page map, and a file header encrypted
//! with a fixed pseudo-random sequence. Layout as AutoCAD writes it; the checksum and header rules follow the ODA
//! specification as implemented by ACadSharp (MIT).

/// A section's content, in file order, with how it is stored.
pub struct Section {
    pub name: &'static str,
    pub data: Vec<u8>,
    pub compressed: bool,
}

/// The pseudo-random sequence (seed 1) used to encrypt the header and pad pages.
fn magic() -> [u8; 256] {
    let mut out = [0u8; 256];
    let mut seed: u32 = 1;
    for b in out.iter_mut() {
        seed = seed.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        *b = (seed >> 16) as u8;
    }
    out
}

/// Adler-style page checksum.
pub fn checksum(seed: u32, data: &[u8]) -> u32 {
    let (mut s1, mut s2) = (seed & 0xFFFF, seed >> 16);
    for chunk in data.chunks(0x15B0) {
        for &b in chunk {
            s1 += b as u32;
            s2 += s1;
        }
        s1 %= 0xFFF1;
        s2 %= 0xFFF1;
    }
    (s2 << 16) | (s1 & 0xFFFF)
}

/// Standard CRC-32 (the file header's).
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

/// LZ77 as AutoCAD 2004 pages use it: literal runs and back-references of up to 0x4000 bytes, greedy matching on
/// 3-byte hash chains. The output ends with the 0x11 terminator. (Inputs of one to three bytes, which DWG pages never
/// are, use the short opening literal code.)
pub fn compress(src: &[u8]) -> Vec<u8> {
    const WINDOW: usize = 0x4000;
    const HASH: usize = 1 << 14;
    let n = src.len();
    let mut head = vec![usize::MAX; HASH];
    let mut prev = vec![usize::MAX; n];
    let hash = |i: usize| ((src[i] as usize) << 8 ^ (src[i + 1] as usize) << 4 ^ src[i + 2] as usize).wrapping_mul(2654435761) >> 7 & (HASH - 1);
    let insert = |i: usize, head: &mut Vec<usize>, prev: &mut Vec<usize>| {
        if i + 3 <= n {
            let h = hash(i);
            prev[i] = head[h];
            head[h] = i;
        }
    };
    // tokens: (literal start, literal length, match length, match offset)
    let mut tokens: Vec<(usize, usize, usize, usize)> = vec![];
    let (mut i, mut lit) = (0usize, 0usize);
    while i < n {
        let mut best = (0usize, 0usize);
        if i + 3 <= n {
            let mut cand = head[hash(i)];
            let mut depth = 0;
            while cand != usize::MAX && i - cand <= WINDOW && depth < 48 {
                let mut l = 0;
                while i + l < n && src[cand + l] == src[i + l] {
                    l += 1;
                }
                if l > best.0 {
                    best = (l, i - cand);
                }
                cand = prev[cand];
                depth += 1;
            }
        }
        // the stream starts with at least four literals: some readers (LibreDWG) have no code for a shorter run there
        if best.0 >= 3 && (i >= 4 || i == n) {
            tokens.push((lit, i - lit, best.0, best.1));
            for k in i..i + best.0 {
                insert(k, &mut head, &mut prev);
            }
            i += best.0;
            lit = i;
        } else {
            insert(i, &mut head, &mut prev);
            i += 1;
        }
    }
    let mut out = vec![];
    // the run of literals before the first match
    let (first_len, rest) = match tokens.first() {
        Some(t) => (t.1, &tokens[..]),
        None => (n - lit, &tokens[..0]),
    };
    let first_start = 0;
    if first_len > 0 && first_len <= 3 {
        out.push(first_len as u8 + 17);
    } else if first_len > 3 {
        literal_code(first_len, &mut out);
    }
    out.extend_from_slice(&src[first_start..first_start + first_len]);
    for (k, t) in rest.iter().enumerate() {
        let (len, off) = (t.2, t.3);
        // the literals after this match: the next token's run, or the tail
        let (after_start, after_len) = match rest.get(k + 1) {
            Some(nx) => (nx.0, nx.1),
            None => (t.0 + t.1 + len, n - (t.0 + t.1 + len)),
        };
        let bits = if (1..=3).contains(&after_len) { after_len as u8 } else { 0 };
        if (3..=14).contains(&len) && off <= 0x400 {
            out.push((((len + 1) << 4) | (((off - 1) & 3) << 2)) as u8 | bits);
            out.push(((off - 1) >> 2) as u8);
        } else {
            let m = len - 2;
            if m <= 0x1F {
                out.push(0x20 | m as u8);
            } else {
                out.push(0x20);
                long_count(m - 0x1F, &mut out);
            }
            out.push(((((off - 1) & 0x3F) << 2) as u8) | bits);
            out.push(((off - 1) >> 6) as u8);
        }
        if after_len > 3 {
            literal_code(after_len, &mut out);
        }
        out.extend_from_slice(&src[after_start..after_start + after_len]);
    }
    out.extend_from_slice(&[0x11, 0, 0]);
    out
}

/// A run of four or more literals.
fn literal_code(count: usize, out: &mut Vec<u8>) {
    let low = count - 3;
    if low <= 0x0F {
        out.push(low as u8);
    } else {
        out.push(0);
        long_count(low - 0x0F, out);
    }
}

/// The extended count: zero bytes worth 0xFF each, then the remainder.
fn long_count(mut r: usize, out: &mut Vec<u8>) {
    while r > 0xFF {
        out.push(0);
        r -= 0xFF;
    }
    out.push(r as u8);
}

struct Page {
    number: u32,
    at: usize,
    size: usize,
}

const DATA: u32 = 0x4163_043B;

/// Assemble the file. `ids` gives each section's id in the section map (AutoCAD numbers them from the end).
pub fn assemble(sections: &[Section], maint: u8) -> Vec<u8> {
    let magic = magic();
    let mut f = vec![0u8; 0x100];
    let mut pages: Vec<Page> = vec![];
    // section descriptors: (name, size, max page size, compressed, id, pages (number, data size, offset))
    struct Desc {
        name: &'static str,
        size: u64,
        max: u32,
        compressed: bool,
        id: u32,
        pages: Vec<(u32, u32, u64)>,
    }
    let n = sections.len() as u32;
    let mut descs: Vec<Desc> = vec![];
    for (k, s) in sections.iter().enumerate() {
        let id = n - k as u32; // the first section written has the highest id
        let max = if s.compressed { 0x7400 } else { s.data.len().max(1).div_ceil(0x100) as u32 * 0x100 };
        let mut d = Desc { name: s.name, size: s.data.len() as u64, max, compressed: s.compressed, id, pages: vec![] };
        let chunks: Vec<&[u8]> = if s.data.is_empty() { vec![&[][..]] } else { s.data.chunks(max as usize).collect() };
        for (c, chunk) in chunks.iter().enumerate() {
            let mut raw = chunk.to_vec();
            raw.resize(max as usize, 0);
            let body = if s.compressed { compress(&raw) } else { raw };
            let at = f.len();
            let page_size = (32 + body.len()).div_ceil(0x20) * 0x20;
            let offset = c as u64 * max as u64;
            let data_sum = checksum(0, &body);
            let mut hdr = Vec::with_capacity(32);
            let header = |hdr: &mut Vec<u8>, sum: u32| {
                hdr.clear();
                hdr.extend_from_slice(&DATA.to_le_bytes());
                hdr.extend_from_slice(&id.to_le_bytes());
                hdr.extend_from_slice(&(body.len() as u32).to_le_bytes());
                hdr.extend_from_slice(&(page_size as u32).to_le_bytes());
                hdr.extend_from_slice(&offset.to_le_bytes());
                hdr.extend_from_slice(&sum.to_le_bytes());
                hdr.extend_from_slice(&data_sum.to_le_bytes());
            };
            header(&mut hdr, 0);
            let sum = checksum(data_sum, &hdr);
            header(&mut hdr, sum);
            let mask = (0x4164_536B_u32 ^ at as u32).to_le_bytes();
            for (i, b) in hdr.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
            f.extend_from_slice(&hdr);
            f.extend_from_slice(&body);
            let pad = page_size - 32 - body.len();
            f.extend_from_slice(&magic[..pad]);
            let number = pages.len() as u32 + 1;
            pages.push(Page { number, at, size: page_size });
            d.pages.push((number, body.len() as u32, offset));
        }
        descs.push(d);
    }
    // section map (descriptors, an empty one first as AutoCAD writes)
    let mut sm = vec![];
    let ndesc = descs.len() as u32 + 1;
    for v in [ndesc, 2, 0x7400, 0, ndesc] {
        sm.extend_from_slice(&v.to_le_bytes());
    }
    let empty = Desc { name: "", size: 0, max: 0x7400, compressed: true, id: 0, pages: vec![] };
    for d in std::iter::once(&empty).chain(descs.iter()) {
        sm.extend_from_slice(&d.size.to_le_bytes());
        for v in [d.pages.len() as u32, d.max, 1, if d.compressed { 2 } else { 1 }, d.id, 0] {
            sm.extend_from_slice(&v.to_le_bytes());
        }
        let mut name = [0u8; 64];
        name[..d.name.len()].copy_from_slice(d.name.as_bytes());
        sm.extend_from_slice(&name);
        for &(num, size, off) in &d.pages {
            sm.extend_from_slice(&num.to_le_bytes());
            sm.extend_from_slice(&size.to_le_bytes());
            sm.extend_from_slice(&off.to_le_bytes());
        }
    }
    let system_page = |f: &mut Vec<u8>, kind: u32, data: &[u8]| -> usize {
        let body = compress(data);
        let mut hdr = vec![];
        let header = |hdr: &mut Vec<u8>, sum: u32| {
            hdr.clear();
            for v in [kind, data.len() as u32, body.len() as u32, 2, sum] {
                hdr.extend_from_slice(&v.to_le_bytes());
            }
        };
        header(&mut hdr, 0);
        let sum = checksum(checksum(0, &hdr), &body);
        header(&mut hdr, sum);
        let at = f.len();
        f.extend_from_slice(&hdr);
        f.extend_from_slice(&body);
        let size = (20 + body.len()).div_ceil(0x20) * 0x20;
        f.extend_from_slice(&magic[..size - 20 - body.len()]);
        at
    };
    let sm_at = system_page(&mut f, 0x4163_003B, &sm);
    let sm_number = pages.len() as u32 + 1;
    pages.push(Page { number: sm_number, at: sm_at, size: f.len() - sm_at });
    // page map: every page with its size, this one included (its size is known only once written, so it is
    // reserved generously: it is the last page, its size only gives the end of the paged area)
    let pm_number = pages.len() as u32 + 1;
    let mut pm = vec![];
    let reserve = (pages.len() + 1) * 8 + 0x40;
    let reserve = (20 + reserve).div_ceil(0x20) * 0x20;
    for p in &pages {
        pm.extend_from_slice(&p.number.to_le_bytes());
        pm.extend_from_slice(&(p.size as u32).to_le_bytes());
    }
    pm.extend_from_slice(&pm_number.to_le_bytes());
    pm.extend_from_slice(&(reserve as u32).to_le_bytes());
    let pm_at = system_page(&mut f, 0x4163_0E3B, &pm);
    let written = f.len() - pm_at;
    debug_assert!(written <= reserve);
    f.resize(pm_at + reserve.max(written), 0);
    let page_end = pm_at + reserve.max(written);
    debug_assert!(pages.iter().all(|p| p.at % 0x20 == 0));
    // the file header block, and its copy at the end
    let second_at = f.len();
    let mut h = vec![];
    h.extend_from_slice(b"AcFssFcAJMB\0");
    let put = |h: &mut Vec<u8>, v: u32| h.extend_from_slice(&v.to_le_bytes());
    put(&mut h, 0);
    put(&mut h, 0x6C);
    put(&mut h, 4);
    put(&mut h, 0); // root tree node gap
    put(&mut h, 0); // left gap
    put(&mut h, 0); // right gap
    put(&mut h, 1);
    put(&mut h, pm_number); // last page id
    h.extend_from_slice(&((page_end - 0x100) as u64).to_le_bytes());
    h.extend_from_slice(&(second_at as u64).to_le_bytes());
    put(&mut h, 0); // gap amount
    put(&mut h, pm_number - 2 + 1); // section page amount
    put(&mut h, 0x20);
    put(&mut h, 0x80);
    put(&mut h, 0x40);
    put(&mut h, pm_number);
    h.extend_from_slice(&((pm_at - 0x100) as u64).to_le_bytes());
    put(&mut h, sm_number);
    put(&mut h, pm_number); // section page array size
    put(&mut h, 0); // gap array size
    put(&mut h, 0); // CRC, computed with these four bytes zero
    let crc = crc32(&h);
    h[0x68..0x6C].copy_from_slice(&crc.to_le_bytes());
    for (i, b) in h.iter_mut().enumerate() {
        *b ^= magic[i];
    }
    f.extend_from_slice(&h);
    // the plain start of the file
    let seek = |name: &str| -> u32 {
        descs.iter().find(|d| d.name == name).and_then(|d| d.pages.first()).and_then(|p| pages.iter().find(|q| q.number == p.0)).map_or(0, |q| q.at as u32 + 0x20)
    };
    let mut head = vec![];
    head.extend_from_slice(b"AC1018");
    head.extend_from_slice(&[0; 5]);
    head.push(maint);
    head.push(3);
    head.extend_from_slice(&seek("AcDb:Preview").to_le_bytes());
    head.push(33); // the version of the program that wrote the file
    head.push(maint);
    head.extend_from_slice(&30u16.to_le_bytes()); // ANSI_1252
    head.extend_from_slice(&[0; 3]);
    head.extend_from_slice(&0u32.to_le_bytes()); // no security
    head.extend_from_slice(&0u32.to_le_bytes());
    head.extend_from_slice(&seek("AcDb:SummaryInfo").to_le_bytes());
    head.extend_from_slice(&0u32.to_le_bytes()); // no VBA project
    head.extend_from_slice(&0x80u32.to_le_bytes());
    head.extend_from_slice(&seek("AcDb:AppInfo").to_le_bytes());
    head.resize(0x80, 0);
    f[..0x80].copy_from_slice(&head);
    f[0x80..0xEC].copy_from_slice(&h);
    f[0xEC..0x100].copy_from_slice(&magic[236..256]);
    f
}

#[cfg(test)]
mod tests {
    use super::super::super::lz77::decompress18;
    use super::*;

    #[test]
    fn compression_round_trips() {
        let mut seed = 7u32;
        let mut rnd = || {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            (seed >> 16) as u8
        };
        let mut cases: Vec<Vec<u8>> = vec![vec![], vec![1], vec![1, 2], vec![1, 2, 3], vec![9; 5], vec![0; 0x7400], (0..=255u8).collect()];
        cases.push((0..5000).map(|i| (i % 7) as u8).collect());
        cases.push((0..0x7400).map(|_| rnd()).collect());
        cases.push((0..0x7400).map(|i| if i % 97 < 60 { b'a' + (i % 13) as u8 } else { rnd() }).collect());
        let text: Vec<u8> = b"LINE 10 20 30 CIRCLE 40 50 LINE 10 20 31 ".iter().cycle().take(20000).copied().collect();
        cases.push(text);
        for c in &cases {
            let z = compress(c);
            assert_eq!(decompress18(&z, c.len() + 16).unwrap(), *c, "{} bytes", c.len());
        }
        // compressible data shrinks
        assert!(compress(&cases[7]).len() < 200);
        assert!(compress(&cases[10]).len() < 600);
    }

    #[test]
    fn checksums_match_autocad() {
        // the file header of AutoCAD's 2004 sample: its CRC-32 is computed with the CRC field zeroed
        let d = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/dwg/samples/sample_AC1018.dwg")).unwrap();
        let m = magic();
        let mut h: Vec<u8> = d[0x80..0xEC].iter().zip(m.iter()).map(|(a, b)| a ^ b).collect();
        let stored = u32::from_le_bytes([h[0x68], h[0x69], h[0x6A], h[0x6B]]);
        h[0x68..0x6C].fill(0);
        assert_eq!(crc32(&h), stored);
        // a data page: the header checksum of AutoCAD's Header page (page 54 at 1094720, see the page map)
        let at = 1_095_008usize;
        let mask = (0x4164_536B_u32 ^ at as u32).to_le_bytes();
        let hdr: Vec<u8> = d[at..at + 32].iter().enumerate().map(|(i, b)| b ^ mask[i % 4]).collect();
        let field = |k: usize| u32::from_le_bytes([hdr[k], hdr[k + 1], hdr[k + 2], hdr[k + 3]]);
        let size = field(8) as usize;
        let data_sum = checksum(0, &d[at + 32..at + 32 + size]);
        assert_eq!(data_sum, field(0x1C));
        let mut zeroed = hdr.clone();
        zeroed[0x18..0x1C].fill(0);
        assert_eq!(checksum(data_sum, &zeroed), field(0x18));
    }
}
