//! The two LZ77 variants DWG uses: one for R2004 and R2010+ pages, another for R2007. Ported from ACadSharp
//! (`DwgLZ77AC18Decompressor`, `DwgLZ77AC21Decompressor`, MIT); every index is checked so damaged data gives an error.

use super::bits::R;

const BAD: &str = "Damaged compressed data in the DWG file.";

struct Src<'a> {
    d: &'a [u8],
    i: usize,
}

impl Src<'_> {
    fn next(&mut self) -> R<u8> {
        let b = *self.d.get(self.i).ok_or(BAD)?;
        self.i += 1;
        Ok(b)
    }
}

/// R2004-style decompression of `src` (it ends with the 0x11 opcode).
pub fn decompress18(src: &[u8], limit: usize) -> R<Vec<u8>> {
    let mut out: Vec<u8> = Vec::with_capacity(limit.min(1 << 24));
    let mut s = Src { d: src, i: 0 };

    fn literal_count(code: u8, s: &mut Src) -> R<usize> {
        let mut low = (code & 0x0F) as usize;
        if low == 0 {
            let mut b = s.next()?;
            while b == 0 {
                low += 0xFF;
                b = s.next()?;
            }
            low += 0x0F + b as usize;
        }
        Ok(low)
    }
    fn copy_lit(n: usize, s: &mut Src, out: &mut Vec<u8>, limit: usize) -> R<u8> {
        let end = s.i.checked_add(n).filter(|&e| e <= s.d.len()).ok_or(BAD)?;
        out.extend_from_slice(&s.d[s.i..end]);
        if out.len() > limit {
            return Err(BAD.into());
        }
        s.i = end;
        s.next()
    }
    fn compressed_bytes(op: u8, valid: u8, s: &mut Src) -> R<usize> {
        let mut n = (op & valid) as usize;
        if n == 0 {
            let mut b = s.next()?;
            while b == 0 {
                n += 0xFF;
                b = s.next()?;
            }
            n += b as usize + valid as usize;
        }
        Ok(n + 2)
    }
    fn two_byte_offset(off: &mut usize, add: usize, s: &mut Src) -> R<u8> {
        let first = s.next()?;
        *off |= (first >> 2) as usize;
        *off |= (s.next()? as usize) << 6;
        *off += add;
        Ok(first)
    }

    let mut op = s.next()?;
    if op > 0x11 {
        op = copy_lit(op as usize - 17, &mut s, &mut out, limit)?;
    }
    if op & 0xF0 == 0 {
        let n = literal_count(op, &mut s)? + 3;
        op = copy_lit(n, &mut s, &mut out, limit)?;
    }
    while op != 0x11 {
        let mut off = 0usize;
        let count;
        if !(0x10..0x40).contains(&op) {
            count = ((op >> 4) as usize).saturating_sub(1);
            let op2 = s.next()?;
            off = (((op >> 2) & 3) as usize | ((op2 as usize) << 2)) + 1;
        } else if op < 0x20 {
            count = compressed_bytes(op, 0x07, &mut s)?;
            off = ((op & 8) as usize) << 11;
            op = two_byte_offset(&mut off, 0x4000, &mut s)?;
        } else {
            count = compressed_bytes(op, 0x1F, &mut s)?;
            op = two_byte_offset(&mut off, 1, &mut s)?;
        }
        if off == 0 || off > out.len() || out.len() + count > limit {
            return Err(BAD.into());
        }
        let start = out.len() - off;
        for k in 0..count {
            let b = out[start + k];
            out.push(b);
        }
        let mut lit = (op & 3) as usize;
        if lit == 0 {
            op = s.next()?;
            if op & 0xF0 == 0 {
                lit = literal_count(op, &mut s)? + 3;
            }
        }
        if lit > 0 {
            op = copy_lit(lit, &mut s, &mut out, limit)?;
        }
    }
    Ok(out)
}

/// R2007-style decompression of `src[start..start + len]` into exactly `out_len` bytes.
pub fn decompress21(src: &[u8], start: usize, len: usize, out_len: usize) -> R<Vec<u8>> {
    let mut d = Dec21 {
        src,
        i: start,
        end: start.checked_add(len).ok_or(BAD)?.min(src.len()),
        op: 0,
        length: 0,
        offset: 0,
        out: vec![0u8; out_len],
        o: 0,
    };
    d.run()?;
    Ok(d.out)
}

struct Dec21<'a> {
    src: &'a [u8],
    i: usize,
    end: usize,
    op: u32,
    length: usize,
    offset: usize,
    out: Vec<u8>,
    o: usize,
}

impl Dec21<'_> {
    fn b(&self, k: usize) -> R<u8> {
        self.src.get(k).copied().ok_or_else(|| BAD.into())
    }
    fn next(&mut self) -> R<u32> {
        let v = self.b(self.i)? as u32;
        self.i += 1;
        Ok(v)
    }

    fn run(&mut self) -> R<()> {
        if self.i >= self.end {
            return Ok(());
        }
        self.op = self.next()?;
        if self.i >= self.end {
            return Ok(());
        }
        if self.op & 0xF0 == 0x20 {
            self.i += 3;
            self.length = (self.b(self.i - 1)? & 7) as usize;
        }
        while self.i < self.end {
            // literal run
            if self.length == 0 {
                self.literal_length()?;
            }
            self.copy_literal(self.length)?;
            self.i += self.length;
            if self.i >= self.end {
                break;
            }
            self.chunks()?;
        }
        Ok(())
    }

    fn literal_length(&mut self) -> R<()> {
        self.length = self.op as usize + 8;
        if self.length == 0x17 {
            let n = self.next()? as usize;
            self.length += n;
            if n == 0xFF {
                loop {
                    let lo = self.next()? as usize;
                    let hi = self.next()? as usize;
                    let n = lo | (hi << 8);
                    self.length += n;
                    if n != 0xFFFF {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    /// The literal bytes are stored in a shuffled order: blocks of 32 as 4-byte groups in the order 24 28 16 20 8 12
    /// 0 4, and the remainder with one of 31 fixed patterns.
    fn copy_literal(&mut self, len: usize) -> R<()> {
        let (mut si, mut di, mut n) = (self.i, self.o, len);
        if si + n > self.src.len() || di + n > self.out.len() {
            return Err(BAD.into());
        }
        while n >= 32 {
            for (k, s) in [24, 28, 16, 20, 8, 12, 0, 4].iter().enumerate() {
                self.out[di + k * 4..di + k * 4 + 4].copy_from_slice(&self.src[si + s..si + s + 4]);
            }
            si += 32;
            di += 32;
            n -= 32;
        }
        if n > 0 {
            for (s, d, cnt) in remainder_pattern(n) {
                self.copy_part(si + s, di + d, cnt);
            }
        }
        self.o += len;
        Ok(())
    }

    /// Copy `cnt` bytes of a pattern piece: 1, 2 and 3 byte pieces are reversed, 4/8/16 byte pieces are split into
    /// 4-byte groups (8 = its two halves swapped, 16 = two 8s swapped).
    fn copy_part(&mut self, s: usize, d: usize, cnt: usize) {
        match cnt {
            1 => self.out[d] = self.src[s],
            2 => {
                self.out[d] = self.src[s + 1];
                self.out[d + 1] = self.src[s];
            }
            3 => {
                self.out[d] = self.src[s + 2];
                self.out[d + 1] = self.src[s + 1];
                self.out[d + 2] = self.src[s];
            }
            4 => self.out[d..d + 4].copy_from_slice(&self.src[s..s + 4]),
            8 => {
                self.out[d..d + 4].copy_from_slice(&self.src[s..s + 4]);
                self.out[d + 4..d + 8].copy_from_slice(&self.src[s + 4..s + 8]);
            }
            16 => {
                self.copy_part(s + 8, d, 8);
                self.copy_part(s, d + 8, 8);
            }
            _ => {}
        }
    }

    fn chunks(&mut self) -> R<()> {
        self.length = 0;
        self.op = self.next()?;
        self.instructions()?;
        loop {
            // back reference
            if self.offset == 0 || self.offset > self.o || self.o + self.length > self.out.len() {
                return Err(BAD.into());
            }
            let from = self.o - self.offset;
            for k in 0..self.length {
                self.out[self.o + k] = self.out[from + k];
            }
            self.o += self.length;
            self.length = (self.op & 7) as usize;
            if self.length != 0 || self.i >= self.end {
                break;
            }
            self.op = self.next()?;
            if self.op >> 4 == 0 {
                break;
            }
            if self.op >> 4 == 15 {
                self.op &= 15;
            }
            self.instructions()?;
        }
        Ok(())
    }

    fn instructions(&mut self) -> R<()> {
        match self.op >> 4 {
            0 => {
                self.length = (self.op & 0xF) as usize + 0x13;
                self.offset = self.next()? as usize;
                self.op = self.next()?;
                self.length += ((self.op >> 3) & 0x10) as usize;
                self.offset += (((self.op & 0x78) as usize) << 5) + 1;
            }
            1 => {
                self.length = (self.op & 0xF) as usize + 3;
                self.offset = self.next()? as usize;
                self.op = self.next()?;
                self.offset += (((self.op & 0xF8) as usize) << 5) + 1;
            }
            2 => {
                self.offset = self.next()? as usize;
                self.offset |= (self.next()? as usize) << 8;
                self.length = (self.op & 7) as usize;
                if self.op & 8 == 0 {
                    self.op = self.next()?;
                    self.length += (self.op & 0xF8) as usize;
                } else {
                    self.offset += 1;
                    self.length += (self.next()? as usize) << 3;
                    self.op = self.next()?;
                    self.length += (((self.op & 0xF8) as usize) << 8) + 0x100;
                }
            }
            _ => {
                self.length = (self.op >> 4) as usize;
                self.offset = (self.op & 15) as usize;
                self.op = self.next()?;
                self.offset += (((self.op & 0xF8) as usize) << 1) + 1;
            }
        }
        Ok(())
    }
}

/// The pieces (source offset, destination offset, size) that copy a literal tail of 1..=31 bytes.
fn remainder_pattern(n: usize) -> Vec<(usize, usize, usize)> {
    match n {
        1 => vec![(0, 0, 1)],
        2 => vec![(0, 0, 2)],
        3 => vec![(0, 0, 3)],
        4 => vec![(0, 0, 4)],
        5 => vec![(4, 0, 1), (0, 1, 4)],
        6 => vec![(5, 0, 1), (1, 1, 4), (0, 5, 1)],
        7 => vec![(5, 0, 2), (1, 2, 4), (0, 6, 1)],
        8 => vec![(0, 0, 8)],
        9 => vec![(8, 0, 1), (0, 1, 8)],
        10 => vec![(9, 0, 1), (1, 1, 8), (0, 9, 1)],
        11 => vec![(9, 0, 2), (1, 2, 8), (0, 10, 1)],
        12 => vec![(8, 0, 4), (0, 4, 8)],
        13 => vec![(12, 0, 1), (8, 1, 4), (0, 5, 8)],
        14 => vec![(13, 0, 1), (9, 1, 4), (1, 5, 8), (0, 13, 1)],
        15 => vec![(13, 0, 2), (9, 2, 4), (1, 6, 8), (0, 14, 1)],
        16 => vec![(0, 0, 16)],
        17 => vec![(9, 0, 8), (8, 8, 1), (0, 9, 8)],
        18 => vec![(17, 0, 1), (1, 1, 16), (0, 17, 1)],
        19 => vec![(16, 0, 3), (0, 3, 16)],
        20 => vec![(16, 0, 4), (8, 4, 8), (0, 12, 8)],
        21 => vec![(20, 0, 1), (16, 1, 4), (8, 5, 8), (0, 13, 8)],
        22 => vec![(20, 0, 2), (16, 2, 4), (8, 6, 8), (0, 14, 8)],
        23 => vec![(20, 0, 3), (16, 3, 4), (8, 7, 8), (0, 15, 8)],
        24 => vec![(16, 0, 8), (0, 8, 16)],
        25 => vec![(17, 0, 8), (16, 8, 1), (0, 9, 16)],
        26 => vec![(25, 0, 1), (17, 1, 8), (16, 9, 1), (0, 10, 16)],
        27 => vec![(25, 0, 2), (17, 2, 8), (16, 10, 1), (0, 11, 16)],
        28 => vec![(24, 0, 4), (16, 4, 8), (8, 12, 8), (0, 20, 8)],
        29 => vec![(28, 0, 1), (24, 1, 4), (16, 5, 8), (8, 13, 8), (0, 21, 8)],
        30 => vec![(28, 0, 2), (24, 2, 4), (16, 6, 8), (8, 14, 8), (0, 22, 8)],
        _ => vec![
            (30, 0, 1),
            (26, 1, 4),
            (18, 5, 8),
            (10, 13, 8),
            (2, 21, 8),
            (0, 29, 2),
        ],
    }
}
