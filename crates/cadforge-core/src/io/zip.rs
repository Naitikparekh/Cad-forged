//! A minimal ZIP writer (stored entries, no compression) for ARCHIVE / ETRANSMIT packages.

fn crc32_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for (i, slot) in t.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *slot = c;
    }
    t
}

/// CRC-32 (IEEE) of `data`.
pub fn crc32(data: &[u8]) -> u32 {
    let t = crc32_table();
    !data.iter().fold(!0u32, |c, b| {
        t[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8)
    })
}

/// A ZIP archive holding the given (path, bytes) files, stored uncompressed. Dated 1980-01-01 so the output is
/// reproducible.
pub fn zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = vec![];
    let mut central: Vec<u8> = vec![];
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend(x.to_le_bytes());
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend(x.to_le_bytes());
    for (name, data) in files {
        let offset = out.len() as u32;
        let (crc, size) = (crc32(data), data.len() as u32);
        let nm = name.as_bytes();
        // local file header
        le32(&mut out, 0x0403_4b50);
        le16(&mut out, 20); // version needed
        le16(&mut out, 0x0800); // UTF-8 names
        le16(&mut out, 0); // stored
        le16(&mut out, 0); // time
        le16(&mut out, 0x21); // date: 1980-01-01
        le32(&mut out, crc);
        le32(&mut out, size);
        le32(&mut out, size);
        le16(&mut out, nm.len() as u16);
        le16(&mut out, 0);
        out.extend(nm);
        out.extend(data);
        // central directory entry
        le32(&mut central, 0x0201_4b50);
        le16(&mut central, 20); // version made by
        le16(&mut central, 20);
        le16(&mut central, 0x0800);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0x21);
        le32(&mut central, crc);
        le32(&mut central, size);
        le32(&mut central, size);
        le16(&mut central, nm.len() as u16);
        le16(&mut central, 0); // extra
        le16(&mut central, 0); // comment
        le16(&mut central, 0); // disk
        le16(&mut central, 0); // internal attributes
        le32(&mut central, 0); // external attributes
        le32(&mut central, offset);
        central.extend(nm);
    }
    let cd_offset = out.len() as u32;
    out.extend(&central);
    le32(&mut out, 0x0605_4b50);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, files.len() as u16);
    le16(&mut out, files.len() as u16);
    le32(&mut out, central.len() as u32);
    le32(&mut out, cd_offset);
    le16(&mut out, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn an_empty_archive_is_just_the_end_record() {
        let z = zip(&[]);
        assert_eq!(z.len(), 22);
        assert_eq!(&z[..4], b"PK\x05\x06");
    }
}
