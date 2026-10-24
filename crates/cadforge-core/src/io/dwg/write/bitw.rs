//! Writing the DWG bit-level types (the inverse of [`super::super::bits::Bits`]), AutoCAD 2000 flavour.

/// A growing bit stream, most significant bit first.
#[derive(Default, Clone)]
pub struct BitW {
    pub buf: Vec<u8>,
    /// Bits written so far.
    pub bits: usize,
}

impl BitW {
    pub fn new() -> BitW {
        BitW::default()
    }

    pub fn bit(&mut self, b: bool) {
        let at = self.bits / 8;
        if at == self.buf.len() {
            self.buf.push(0);
        }
        if b {
            self.buf[at] |= 0x80 >> (self.bits % 8);
        }
        self.bits += 1;
    }

    fn put(&mut self, v: u64, n: u32) {
        for i in (0..n).rev() {
            self.bit((v >> i) & 1 == 1);
        }
    }

    /// BB: two bits.
    pub fn bb(&mut self, v: u8) {
        self.put(v as u64, 2);
    }

    /// RC: a raw byte.
    pub fn rc(&mut self, v: u8) {
        if self.bits.is_multiple_of(8) {
            self.buf.push(v);
            self.bits += 8;
        } else {
            self.put(v as u64, 8);
        }
    }

    pub fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.rc(x);
        }
    }

    pub fn rs(&mut self, v: i16) {
        self.bytes(&v.to_le_bytes());
    }

    pub fn rl(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes());
    }

    pub fn rd(&mut self, v: f64) {
        self.bytes(&v.to_le_bytes());
    }

    pub fn rd2(&mut self, p: [f64; 2]) {
        self.rd(p[0]);
        self.rd(p[1]);
    }

    /// BS: bit-coded short.
    pub fn bs(&mut self, v: i16) {
        match v {
            0 => self.bb(2),
            256 => self.bb(3),
            1..=255 => {
                self.bb(1);
                self.rc(v as u8);
            }
            _ => {
                self.bb(0);
                self.rs(v);
            }
        }
    }

    /// BL: bit-coded long.
    pub fn bl(&mut self, v: i32) {
        match v {
            0 => self.bb(2),
            1..=255 => {
                self.bb(1);
                self.rc(v as u8);
            }
            _ => {
                self.bb(0);
                self.rl(v);
            }
        }
    }

    /// BD: bit-coded double.
    pub fn bd(&mut self, v: f64) {
        let v = if v.is_finite() { v } else { 0.0 };
        if v == 0.0 && v.is_sign_positive() {
            self.bb(2);
        } else if v == 1.0 {
            self.bb(1);
        } else {
            self.bb(0);
            self.rd(v);
        }
    }

    pub fn bd2(&mut self, p: [f64; 2]) {
        self.bd(p[0]);
        self.bd(p[1]);
    }

    pub fn bd3(&mut self, p: [f64; 3]) {
        self.bd(p[0]);
        self.bd(p[1]);
        self.bd(p[2]);
    }

    /// DD: a double stored relative to a default value (only the bytes that differ).
    pub fn dd(&mut self, v: f64, def: f64) {
        let v = if v.is_finite() { v } else { 0.0 };
        let (a, d) = (v.to_le_bytes(), def.to_le_bytes());
        if a == d {
            self.bb(0);
        } else if a[4..8] == d[4..8] {
            self.bb(1);
            self.bytes(&a[0..4]);
        } else if a[6..8] == d[6..8] {
            self.bb(2);
            self.bytes(&a[4..6]);
            self.bytes(&a[0..4]);
        } else {
            self.bb(3);
            self.rd(v);
        }
    }

    /// BT: thickness (a set bit means 0).
    pub fn bt(&mut self, v: f64) {
        if v == 0.0 {
            self.bit(true);
        } else {
            self.bit(false);
            self.bd(v);
        }
    }

    /// BE: extrusion (a set bit means 0,0,1).
    pub fn be(&mut self, n: [f64; 3]) {
        if n == [0.0, 0.0, 1.0] {
            self.bit(true);
        } else {
            self.bit(false);
            self.bd3(n);
        }
    }

    /// H: a handle reference with its reference code (2 soft owner, 3 hard owner, 4 soft pointer, 5 hard pointer).
    pub fn h(&mut self, code: u8, handle: u64) {
        let n = (8 - handle.leading_zeros() as usize / 8).min(8);
        let n = if handle == 0 { 0 } else { n };
        self.rc((code << 4) | n as u8);
        for i in (0..n).rev() {
            self.rc((handle >> (i * 8)) as u8);
        }
    }

    /// TV: text (R2000: a BS length and the code-page bytes; CadForge only writes ASCII, other characters as \U+XXXX).
    pub fn tv(&mut self, s: &str) {
        let b: Vec<u8> = s.chars().map(|c| if (c as u32) < 0x100 { c as u8 } else { b'?' }).collect();
        self.bs(b.len().min(i16::MAX as usize) as i16);
        self.bytes(&b[..b.len().min(i16::MAX as usize)]);
    }

    /// Pad with zero bits to a byte boundary.
    pub fn align(&mut self) {
        while !self.bits.is_multiple_of(8) {
            self.bit(false);
        }
    }

    /// Append another stream bit by bit (the handle stream after an object's data).
    pub fn append(&mut self, o: &BitW) {
        if self.bits.is_multiple_of(8) {
            self.buf.truncate(self.bits / 8);
            self.buf.extend_from_slice(&o.buf);
            self.bits += o.bits;
            self.buf.truncate(self.bits.div_ceil(8));
            return;
        }
        for i in 0..o.bits {
            self.bit(o.buf[i / 8] & (0x80 >> (i % 8)) != 0);
        }
    }

    /// Overwrite 32 bits at a bit position with a raw long (the object size, known only at the end).
    pub fn patch_rl(&mut self, at: usize, v: u32) {
        for (k, byte) in v.to_le_bytes().iter().enumerate() {
            for i in 0..8 {
                let pos = at + k * 8 + i;
                let mask = 0x80 >> (pos % 8);
                if byte & (0x80 >> i) != 0 {
                    self.buf[pos / 8] |= mask;
                } else {
                    self.buf[pos / 8] &= !mask;
                }
            }
        }
    }
}

const fn crc_table() -> [u16; 256] {
    let mut t = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u16;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xA001 } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

const CRC: [u16; 256] = crc_table();

/// The DWG CRC-16 (seed 0xC0C1 for sections and objects).
pub fn crc16(seed: u16, data: &[u8]) -> u16 {
    let mut k = seed;
    for &b in data {
        k = (k >> 8) ^ CRC[((b as u16 ^ k) & 0xFF) as usize];
    }
    k
}

#[cfg(test)]
mod tests {
    use super::super::super::bits::Bits;
    use super::super::super::Ver;
    use super::*;

    #[test]
    fn round_trip_through_the_reader() {
        let mut w = BitW::new();
        w.bit(true);
        w.bs(0);
        w.bs(7);
        w.bs(256);
        w.bs(-5);
        w.bl(0);
        w.bl(200);
        w.bl(70000);
        w.bd(0.0);
        w.bd(1.0);
        w.bd(2.5);
        w.dd(3.25, 3.0);
        w.dd(3.0, 3.0);
        w.dd(1e10, 1e10 + 1.0);
        w.dd(-7.0, 7.0);
        w.bt(0.0);
        w.bt(2.0);
        w.be([0.0, 0.0, 1.0]);
        w.be([0.0, 1.0, 0.0]);
        w.h(5, 0x1A2B);
        w.h(4, 0);
        w.tv("Layer 1");
        w.rd(1.5);
        let mut r = Bits::new(&w.buf, Ver::R2000);
        assert!(r.bit().unwrap());
        assert_eq!([r.bs().unwrap(), r.bs().unwrap(), r.bs().unwrap(), r.bs().unwrap()], [0, 7, 256, -5]);
        assert_eq!([r.bl().unwrap(), r.bl().unwrap(), r.bl().unwrap()], [0, 200, 70000]);
        assert_eq!([r.bd().unwrap(), r.bd().unwrap(), r.bd().unwrap()], [0.0, 1.0, 2.5]);
        assert_eq!(r.dd(3.0).unwrap(), 3.25);
        assert_eq!(r.dd(3.0).unwrap(), 3.0);
        assert_eq!(r.dd(1e10 + 1.0).unwrap(), 1e10);
        assert_eq!(r.dd(7.0).unwrap(), -7.0);
        assert_eq!(r.bt().unwrap(), 0.0);
        assert_eq!(r.bt().unwrap(), 2.0);
        assert_eq!(r.be().unwrap(), [0.0, 0.0, 1.0]);
        assert_eq!(r.be().unwrap(), [0.0, 1.0, 0.0]);
        assert_eq!(r.handle(0).unwrap(), 0x1A2B);
        assert_eq!(r.handle(0).unwrap(), 0);
        assert_eq!(r.tv().unwrap(), "Layer 1");
        assert_eq!(r.rd().unwrap(), 1.5);
    }

    #[test]
    fn append_and_patch() {
        let mut a = BitW::new();
        a.bit(true);
        a.rl(0);
        let mut b = BitW::new();
        b.bb(3);
        b.rc(0xAB);
        a.append(&b);
        a.patch_rl(1, 0xDEADBEEF);
        let mut r = Bits::new(&a.buf, Ver::R2000);
        assert!(r.bit().unwrap());
        assert_eq!(r.rl().unwrap(), 0xDEADBEEF);
        assert_eq!(r.bb().unwrap(), 3);
        assert_eq!(r.rc().unwrap(), 0xAB);
        assert_eq!(a.bits, 1 + 32 + 10);
    }

    #[test]
    fn crc_matches_autocad() {
        // the file header of the AutoCAD 2000 sample: CRC of its first 0x4F bytes is stored right after them
        let d = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/dwg/samples/sample_AC1015.dwg")).unwrap();
        assert_eq!(crc16(0xC0C1, &d[..0x4F]), u16::from_le_bytes([d[0x4F], d[0x50]]));
    }
}
