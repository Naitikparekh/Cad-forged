//! Binary DXF: the same groups as the text format, packed as a group code followed by a typed value.
//!
//! The file starts with the 22 byte sentinel `AutoCAD Binary DXF\r\n\x1a\0`. For R2000 and later every group code is a
//! little-endian 16 bit number (R12 and older use one byte, with 255 introducing a two byte code for extended data).
//! The value that follows has a type decided by the code: 16, 32 or 64 bit integers, an 8 byte double, one byte for the
//! 290-299 flags, a length-prefixed chunk of bytes for 310-319 and 1004, and a zero terminated string for the rest.
//!
//! Reading unpacks the groups into DXF text, which the ordinary reader then parses; writing packs the text our writer
//! produces. That keeps one parser and one writer, and the two conversions are small enough to check against ezdxf.

/// The first 22 bytes of every binary DXF file.
pub const SENTINEL: &[u8; 22] = b"AutoCAD Binary DXF\r\n\x1a\0";

/// Whether `bytes` start with the binary DXF sentinel.
pub fn is_binary_dxf(bytes: &[u8]) -> bool {
    bytes.starts_with(SENTINEL)
}

#[derive(PartialEq, Clone, Copy)]
enum Ty {
    Chunk,
    Flag,
    I16,
    I32,
    I64,
    F64,
    Str,
}

fn type_of(code: i32) -> Ty {
    match code {
        310..=319 | 1004 => Ty::Chunk,
        290..=299 => Ty::Flag,
        60..=79 | 170..=179 | 270..=289 | 370..=389 | 400..=409 | 1060..=1070 => Ty::I16,
        90..=99 | 420..=429 | 440..=459 | 1071 => Ty::I32,
        160..=169 => Ty::I64,
        10..=59 | 110..=149 | 210..=239 | 460..=469 | 1010..=1059 => Ty::F64,
        _ => Ty::Str,
    }
}

const TRUNCATED: &str = "The binary DXF file ends in the middle of a value.";

/// A bounds-checked cursor over the file.
struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.i.checked_add(n).filter(|&e| e <= self.b.len()).ok_or(TRUNCATED)?;
        let s = &self.b[self.i..end];
        self.i = end;
        Ok(s)
    }
    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.take(N)?);
        Ok(a)
    }
    fn cstring(&mut self) -> Result<&'a [u8], String> {
        let rest = &self.b[self.i.min(self.b.len())..];
        let n = rest.iter().position(|&c| c == 0).ok_or(TRUNCATED)?;
        let s = &rest[..n];
        self.i += n + 1;
        Ok(s)
    }
}

/// The `$ACADVER` value, found the way other readers find it: by scanning the start of the file for the variable name.
fn version_of(b: &[u8]) -> String {
    let head = &b[SENTINEL.len().min(b.len())..b.len().min(1024)];
    let Some(at) = head.windows(8).position(|w| w == b"$ACADVER") else { return "AC1009".into() };
    // name, its terminating zero, then the group code (one or two bytes) of the value
    let mut s = at + 10;
    if head.get(s) != Some(&b'A') {
        s += 1;
    }
    head.get(s..s + 6).map(|v| String::from_utf8_lossy(v).into_owned()).unwrap_or_else(|| "AC1009".into())
}

/// Convert a binary DXF file to DXF text.
pub fn binary_to_text(bytes: &[u8]) -> Result<String, String> {
    if !is_binary_dxf(bytes) {
        return Err("Not a binary DXF file.".into());
    }
    let version = version_of(bytes);
    let r12 = version.as_str() <= "AC1009";
    let utf8 = version.as_str() >= "AC1021";
    let mut c = Cursor { b: bytes, i: SENTINEL.len() };
    let mut out = String::with_capacity(bytes.len() * 2);
    while c.i < bytes.len() {
        let code = if r12 {
            match c.byte()? {
                255 => u16::from_le_bytes(c.array()?) as i32,
                v => v as i32,
            }
        } else {
            u16::from_le_bytes(c.array()?) as i32
        };
        out.push_str(&code.to_string());
        out.push('\n');
        match type_of(code) {
            Ty::Chunk => {
                let n = c.byte()? as usize;
                for v in c.take(n)? {
                    out.push_str(&format!("{v:02X}"));
                }
            }
            Ty::Flag => out.push_str(&c.byte()?.to_string()),
            Ty::I16 => out.push_str(&i16::from_le_bytes(c.array()?).to_string()),
            Ty::I32 => out.push_str(&i32::from_le_bytes(c.array()?).to_string()),
            Ty::I64 => out.push_str(&i64::from_le_bytes(c.array()?).to_string()),
            Ty::F64 => {
                let v = f64::from_le_bytes(c.array()?);
                out.push_str(&if v.is_finite() { v.to_string() } else { "0".into() });
            }
            Ty::Str => {
                let s = c.cstring()?;
                if utf8 {
                    out.push_str(&String::from_utf8_lossy(s));
                } else {
                    out.push_str(&super::dxf_read::cp1252_to_string(s));
                }
            }
        }
        out.push('\n');
    }
    Ok(out)
}

/// Pack DXF text (as our writer produces it) as binary DXF, R2000 style with two byte group codes.
pub fn text_to_binary(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() / 2);
    out.extend_from_slice(SENTINEL);
    let mut lines = text.split('\n').map(|l| l.trim_end_matches('\r'));
    while let (Some(c), Some(v)) = (lines.next(), lines.next()) {
        let Ok(code) = c.trim().parse::<i32>() else { continue };
        let Ok(code16) = u16::try_from(code) else { continue };
        let ty = type_of(code);
        let value = v.trim();
        if ty == Ty::Chunk {
            // hex text -> at most 127 bytes per group
            let bytes: Vec<u8> = (0..value.len() / 2).filter_map(|k| value.get(2 * k..2 * k + 2).and_then(|h| u8::from_str_radix(h, 16).ok())).collect();
            for chunk in bytes.chunks(127) {
                out.extend_from_slice(&code16.to_le_bytes());
                out.push(chunk.len() as u8);
                out.extend_from_slice(chunk);
            }
            continue;
        }
        out.extend_from_slice(&code16.to_le_bytes());
        match ty {
            Ty::Flag => out.push(value.parse::<i64>().unwrap_or(0).clamp(0, 255) as u8),
            Ty::I16 => out.extend_from_slice(&(value.parse::<i64>().unwrap_or(0).clamp(i16::MIN as i64, i16::MAX as i64) as i16).to_le_bytes()),
            Ty::I32 => out.extend_from_slice(&(value.parse::<i64>().unwrap_or(0).clamp(i32::MIN as i64, i32::MAX as i64) as i32).to_le_bytes()),
            Ty::I64 => out.extend_from_slice(&value.parse::<i64>().unwrap_or(0).to_le_bytes()),
            Ty::F64 => out.extend_from_slice(&value.parse::<f64>().unwrap_or(0.0).to_le_bytes()),
            Ty::Str | Ty::Chunk => {
                // R2000 strings are code page 1252; the writer has already escaped anything outside ASCII
                out.extend(v.trim_end_matches('\r').chars().map(|ch| if (ch as u32) < 256 { ch as u8 } else { b'?' }));
                out.push(0);
            }
        }
    }
    out
}
