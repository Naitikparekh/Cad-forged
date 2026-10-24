//! Reading DWG's bit-packed values: everything is addressed in bits, most significant bit first, and the compound types
//! (BS, BL, BD, DD, BT, BE, MC, handles, text, colours) are built from a few-bit prefix followed by raw bytes.
//! Every read is bounds-checked: a truncated or damaged file gives an error, never a panic.

use super::Ver;

pub type R<T> = Result<T, String>;

pub const EOF: &str = "The DWG file ends in the middle of an object.";

#[derive(Clone)]
pub struct Bits<'a> {
    pub d: &'a [u8],
    /// Position in bits.
    pub pos: usize,
    pub ver: Ver,
}

impl<'a> Bits<'a> {
    pub fn new(d: &'a [u8], ver: Ver) -> Self {
        Bits { d, pos: 0, ver }
    }

    pub fn at(d: &'a [u8], ver: Ver, pos_bits: usize) -> Self {
        Bits {
            d,
            pos: pos_bits,
            ver,
        }
    }

    pub fn byte_pos(&self) -> usize {
        self.pos.div_ceil(8)
    }

    pub fn len_bits(&self) -> usize {
        self.d.len() * 8
    }

    pub fn bit(&mut self) -> R<bool> {
        let i = self.pos >> 3;
        let b = *self.d.get(i).ok_or(EOF)?;
        let v = (b >> (7 - (self.pos & 7))) & 1 == 1;
        self.pos += 1;
        Ok(v)
    }

    /// `n` bits (at most 64), first bit read is the most significant.
    pub fn bits(&mut self, n: usize) -> R<u64> {
        if self.pos + n > self.len_bits() {
            return Err(EOF.into());
        }
        let mut v = 0u64;
        // fast path for whole bytes on a byte boundary
        if self.pos & 7 == 0 && n & 7 == 0 {
            for k in 0..n / 8 {
                v = (v << 8) | self.d[(self.pos >> 3) + k] as u64;
            }
            self.pos += n;
            return Ok(v);
        }
        for _ in 0..n {
            v = (v << 1) | self.bit()? as u64;
        }
        Ok(v)
    }

    /// BB: two bits.
    pub fn bb(&mut self) -> R<u8> {
        Ok(self.bits(2)? as u8)
    }

    /// RC: a raw byte.
    pub fn rc(&mut self) -> R<u8> {
        Ok(self.bits(8)? as u8)
    }

    pub fn bytes(&mut self, n: usize) -> R<Vec<u8>> {
        if self.pos + n * 8 > self.len_bits() {
            return Err(EOF.into());
        }
        if self.pos & 7 == 0 {
            let i = self.pos >> 3;
            self.pos += n * 8;
            return Ok(self.d[i..i + n].to_vec());
        }
        (0..n).map(|_| self.rc()).collect()
    }

    pub fn skip_bytes(&mut self, n: usize) -> R<()> {
        if self.pos + n * 8 > self.len_bits() {
            return Err(EOF.into());
        }
        self.pos += n * 8;
        Ok(())
    }

    /// RS: little-endian 16 bits.
    pub fn rs(&mut self) -> R<u16> {
        let a = self.rc()? as u16;
        let b = self.rc()? as u16;
        Ok(a | (b << 8))
    }

    /// RL: little-endian 32 bits.
    pub fn rl(&mut self) -> R<u32> {
        let mut v = 0u32;
        for k in 0..4 {
            v |= (self.rc()? as u32) << (8 * k);
        }
        Ok(v)
    }

    /// RD: little-endian IEEE double.
    pub fn rd(&mut self) -> R<f64> {
        let mut a = [0u8; 8];
        for x in a.iter_mut() {
            *x = self.rc()?;
        }
        Ok(f64::from_le_bytes(a))
    }

    pub fn rd2(&mut self) -> R<[f64; 2]> {
        Ok([self.rd()?, self.rd()?])
    }

    pub fn rd3(&mut self) -> R<[f64; 3]> {
        Ok([self.rd()?, self.rd()?, self.rd()?])
    }

    /// BS: bit short.
    pub fn bs(&mut self) -> R<i16> {
        Ok(match self.bb()? {
            0 => self.rs()? as i16,
            1 => self.rc()? as i16,
            2 => 0,
            _ => 256,
        })
    }

    /// BL: bit long.
    pub fn bl(&mut self) -> R<i32> {
        match self.bb()? {
            0 => Ok(self.rl()? as i32),
            1 => Ok(self.rc()? as i32),
            2 => Ok(0),
            _ => Err("invalid bit long".into()),
        }
    }

    /// BLL: bit long long (3-bit byte count, then little-endian bytes).
    pub fn bll(&mut self) -> R<u64> {
        let n = self.bits(3)? as usize;
        let mut v = 0u64;
        for k in 0..n {
            v |= (self.rc()? as u64) << (8 * k);
        }
        Ok(v)
    }

    /// BD: bit double.
    pub fn bd(&mut self) -> R<f64> {
        match self.bb()? {
            0 => self.rd(),
            1 => Ok(1.0),
            2 => Ok(0.0),
            _ => Err("invalid bit double".into()),
        }
    }

    pub fn bd2(&mut self) -> R<[f64; 2]> {
        Ok([self.bd()?, self.bd()?])
    }

    pub fn bd3(&mut self) -> R<[f64; 3]> {
        Ok([self.bd()?, self.bd()?, self.bd()?])
    }

    /// DD: a double stored as a patch over a default value.
    pub fn dd(&mut self, def: f64) -> R<f64> {
        let mut a = def.to_le_bytes();
        match self.bb()? {
            0 => Ok(def),
            1 => {
                for x in a.iter_mut().take(4) {
                    *x = self.rc()?;
                }
                Ok(f64::from_le_bytes(a))
            }
            2 => {
                a[4] = self.rc()?;
                a[5] = self.rc()?;
                for x in a.iter_mut().take(4) {
                    *x = self.rc()?;
                }
                Ok(f64::from_le_bytes(a))
            }
            _ => self.rd(),
        }
    }

    pub fn dd2(&mut self, def: [f64; 2]) -> R<[f64; 2]> {
        Ok([self.dd(def[0])?, self.dd(def[1])?])
    }

    pub fn dd3(&mut self, def: [f64; 3]) -> R<[f64; 3]> {
        Ok([self.dd(def[0])?, self.dd(def[1])?, self.dd(def[2])?])
    }

    /// BT: thickness (R2000+: a bit says "zero").
    pub fn bt(&mut self) -> R<f64> {
        if self.ver >= Ver::R2000 && self.bit()? {
            return Ok(0.0);
        }
        self.bd()
    }

    /// BE: extrusion (R2000+: a bit says "0,0,1").
    pub fn be(&mut self) -> R<[f64; 3]> {
        if self.ver >= Ver::R2000 && self.bit()? {
            return Ok([0.0, 0.0, 1.0]);
        }
        self.bd3()
    }

    /// MC: unsigned modular char.
    pub fn mc(&mut self) -> R<u64> {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let b = self.rc()?;
            if shift < 63 {
                v |= ((b & 0x7F) as u64) << shift;
            }
            if b & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
            if shift > 70 {
                return Err("modular char too long".into());
            }
        }
    }

    /// Signed modular char: the last byte carries 6 value bits and the sign in 0x40.
    pub fn smc(&mut self) -> R<i64> {
        let mut v = 0i64;
        let mut shift = 0;
        loop {
            let b = self.rc()?;
            if b & 0x80 != 0 {
                if shift < 63 {
                    v |= ((b & 0x7F) as i64) << shift;
                }
                shift += 7;
                if shift > 70 {
                    return Err("modular char too long".into());
                }
                continue;
            }
            if shift < 63 {
                v |= ((b & 0x3F) as i64) << shift;
            }
            return Ok(if b & 0x40 != 0 { -v } else { v });
        }
    }

    /// MS: modular short (object sizes).
    pub fn ms(&mut self) -> R<u64> {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let lo = self.rc()? as u64;
            let hi = self.rc()? as u64;
            if shift < 60 {
                v |= (lo | ((hi & 0x7F) << 8)) << shift;
            }
            if hi & 0x80 == 0 {
                return Ok(v);
            }
            shift += 15;
            if shift > 75 {
                return Err("modular short too long".into());
            }
        }
    }

    /// H: a handle reference, resolved against `reference` for the relative codes.
    pub fn handle(&mut self, reference: u64) -> R<u64> {
        let form = self.rc()?;
        let code = form >> 4;
        let n = (form & 0x0F) as usize;
        let mut v = 0u64;
        for _ in 0..n {
            v = (v << 8) | self.rc()? as u64;
        }
        Ok(match code {
            0..=5 => v,
            6 => reference.wrapping_add(1),
            8 => reference.wrapping_sub(1),
            0xA => reference.wrapping_add(v),
            0xC => reference.wrapping_sub(v),
            _ => return Err(format!("invalid handle reference code {code}")),
        })
    }

    /// TV: variable text (code page bytes before R2007, UTF-16 from R2007 on).
    pub fn tv(&mut self) -> R<String> {
        let n = self.bs()?;
        if n <= 0 {
            return Ok(String::new());
        }
        let n = n as u16 as usize;
        if self.ver >= Ver::R2007 {
            let raw = self.bytes(n * 2)?;
            let units: Vec<u16> = raw
                .chunks(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            Ok(String::from_utf16_lossy(&units).replace('\0', ""))
        } else {
            let raw = self.bytes(n)?;
            Ok(crate::io::dxf_read::cp1252_to_string(&raw).replace('\0', ""))
        }
    }

    /// OT: object type (R2010+ has its own compact form).
    pub fn ot(&mut self) -> R<u16> {
        if self.ver >= Ver::R2010 {
            return Ok(match self.bb()? {
                0 => self.rc()? as u16,
                1 => 0x1F0 + self.rc()? as u16,
                _ => self.rs()?,
            });
        }
        Ok(self.bs()? as u16)
    }

    /// Move to the start of an object's string stream, R2007+: `end` is the bit just before the handles, which holds
    /// the "has strings" flag; the size of the string data sits in the 16 (or 32) bits before it.
    pub fn set_string_stream(&mut self, end: i64) -> R<bool> {
        if end < 0 {
            return Ok(false);
        }
        self.pos = end as usize;
        if !self.bit()? {
            return Ok(false);
        }
        let mut len = end - 16;
        if len < 0 {
            return Err(EOF.into());
        }
        self.pos = len as usize;
        let mut size = self.rs()? as i64;
        if size & 0x8000 != 0 {
            len -= 16;
            if len < 0 {
                return Err(EOF.into());
            }
            self.pos = len as usize;
            size &= 0x7FFF;
            let hi = self.rs()? as i64;
            size += hi << 15;
        }
        let start = len - size;
        if start < 0 {
            return Err(EOF.into());
        }
        self.pos = start as usize;
        Ok(true)
    }
}
