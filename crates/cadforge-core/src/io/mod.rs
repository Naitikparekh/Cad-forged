//! File formats: DXF read/write and CadForge project files.
pub mod binary_dxf;
pub mod dwg;
pub mod dxf_read;
pub mod dxf_write;
pub mod project;
pub mod zip;

pub use binary_dxf::is_binary_dxf;
pub use dwg::{is_dwg, read_dwg};
pub use dxf_read::{decode_dxf_bytes, dxf_text_from_bytes, read_dxf, read_dxf_bytes};
pub use dxf_write::{write_dxf, write_dxf_binary};
pub use project::{from_json, strip_ext, to_json, FILE_EXT};
