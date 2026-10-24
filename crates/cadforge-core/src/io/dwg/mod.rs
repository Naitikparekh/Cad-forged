//! DWG, AutoCAD's native format: reading R13 to 2018 files.
//!
//! The bit-level format, the section layouts of each version and the field order of every object are ported from
//! ACadSharp (<https://github.com/DomCR/ACadSharp>, MIT License, Copyright (c) 2021 Albert Domenech). The objects are
//! turned into DXF groups and read by the DXF reader, so both formats share one importer.

pub mod bits;
mod file;
mod header;
mod lz77;
mod objects;
mod to_dxf;
pub mod write;

pub use write::{dxf_to_dwg, write_dwg, write_dwg_as};

use crate::drawing::Drawing;
use std::collections::BTreeMap;

/// DWG versions, oldest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ver {
    R13,
    R14,
    R2000,
    R2004,
    R2007,
    R2010,
    R2013,
    R2018,
}

impl Ver {
    pub fn label(self) -> &'static str {
        match self {
            Ver::R13 => "R13",
            Ver::R14 => "R14",
            Ver::R2000 => "2000",
            Ver::R2004 => "2004",
            Ver::R2007 => "2007",
            Ver::R2010 => "2010",
            Ver::R2013 => "2013",
            Ver::R2018 => "2018",
        }
    }
}

/// Whether `bytes` look like a DWG file (any version, supported or not).
pub fn is_dwg(bytes: &[u8]) -> bool {
    bytes.len() >= 6 && (bytes.starts_with(b"AC10") || bytes.starts_with(b"AC1."))
}

/// What happened while reading a DWG file.
#[derive(Debug, Clone, Default)]
pub struct DwgReport {
    pub version: &'static str,
    pub objects: usize,
    /// Objects that could not be decoded (damaged, or a variant not understood), by type.
    pub failed: BTreeMap<String, usize>,
    /// Entities of types CadForge does not use, by DXF name.
    pub unsupported: BTreeMap<String, usize>,
}

/// Convert a DWG file to DXF text (plus a report of what could not be read).
pub fn dwg_to_dxf(bytes: &[u8]) -> Result<(String, DwgReport), String> {
    let f = file::DwgFile::open(bytes)?;
    let hdr = f
        .section("AcDb:Header")
        .map(|d| header::read(d, f.ver, f.maint))
        .unwrap_or_default();
    let classes = f.classes();
    let map = f.handle_map()?;
    let data = f.objects().ok_or("The DWG file has no objects section.")?;
    let names: std::collections::HashMap<u16, String> =
        classes.iter().map(|(k, (n, _))| (*k, n.clone())).collect();
    let mut rep = DwgReport {
        version: f.ver.label(),
        ..Default::default()
    };
    let mut objs = Vec::with_capacity(map.len());
    for (_, off) in &map {
        if *off >= data.len() {
            *rep.failed.entry("(outside the file)".into()).or_default() += 1;
            continue;
        }
        match objects::read_object(data, f.ver, *off, &names) {
            Ok(Some(o)) => {
                if matches!(o.ty, "REGION" | "3DSOLID") {
                    *rep.unsupported.entry(o.ty.to_string()).or_default() += 1;
                } else {
                    objs.push(o);
                }
            }
            Ok(None) => {
                if let Some(t) = objects::entity_type_name(data, f.ver, *off, &classes) {
                    *rep.unsupported.entry(t).or_default() += 1;
                }
            }
            Err(e) => {
                let t = objects::type_name(data, f.ver, *off, &names).unwrap_or_else(|| "?".into());
                *rep.failed.entry(t).or_default() += 1;
                let _ = e;
            }
        }
    }
    rep.objects = objs.len();
    objs.sort_by_key(|o| o.handle);
    objs.dedup_by_key(|o| o.handle);
    Ok((to_dxf::emit(&objs, &hdr), rep))
}

/// Every object in a DWG file as (handle, type name), in handle order: for diagnostics.
pub fn list_objects(bytes: &[u8]) -> Result<Vec<(u64, String)>, String> {
    let f = file::DwgFile::open(bytes)?;
    let classes = f.classes();
    let names: std::collections::HashMap<u16, String> =
        classes.iter().map(|(k, (n, _))| (*k, n.clone())).collect();
    let data = f.objects().ok_or("The DWG file has no objects section.")?;
    let mut out: Vec<(u64, String)> = f
        .handle_map()?
        .into_iter()
        .map(|(h, off)| {
            (
                h,
                objects::type_name(data, f.ver, off, &names).unwrap_or_else(|| "?".into()),
            )
        })
        .collect();
    out.sort();
    Ok(out)
}

/// Read a DWG file into a drawing. Objects that could not be read, and entity types CadForge does not use, are listed
/// in [`Drawing::skipped`].
pub fn read_dwg(bytes: &[u8], name: &str) -> Result<Drawing, String> {
    let (text, rep) = dwg_to_dxf(bytes)?;
    let mut d = crate::io::read_dxf(&text, name)?;
    // saving keeps the file readable by what read it: AutoCAD 2000 for 2000 and older files
    if file::version_of(bytes).is_some_and(|v| v <= Ver::R2000) {
        d.dwg_version = Some(Ver::R2000);
    }
    for (k, n) in rep.unsupported {
        *d.skipped.entry(k).or_insert(0) += n;
    }
    for (k, n) in rep.failed {
        *d.skipped.entry(format!("{k} (unreadable)")).or_insert(0) += n;
    }
    Ok(d)
}
