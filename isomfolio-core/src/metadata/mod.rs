pub mod xmp;
pub mod exif;

#[cfg(target_os = "macos")]
pub mod apple;

use std::path::Path;
use crate::models::ExifTechMeta;

pub use xmp::{XmpCore, XmpMetadata, DublinCore};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AppleTag {
    pub text: String,
    pub color_idx: i32,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AppleMetadata {
    pub user_tags: Vec<AppleTag>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct EmbeddedMetadata {
    pub xmp: Option<XmpMetadata>,
    pub apple: Option<AppleMetadata>,
    pub exif_tech: Option<ExifTechMeta>,
}

pub fn read_metadata(file_path: &str) -> EmbeddedMetadata {
    let bytes = std::fs::read(file_path).ok();
    let exif = bytes.as_deref().and_then(exif::read_exif_from_bytes);
    read_metadata_from(file_path, bytes.as_deref(), exif.as_ref())
}

/// Build embedded metadata from already-read image bytes and an already-parsed
/// EXIF block, so one `fs::read` + one EXIF parse feed both the file-identity
/// stage and this metadata stage during a scan (no redundant opens/parses).
/// `bytes` is `None` when the file was unreadable; `exif` is `None` when absent.
pub fn read_metadata_from(
    file_path: &str,
    bytes: Option<&[u8]>,
    exif: Option<&exif::ExifData>,
) -> EmbeddedMetadata {
    let sidecar_path = {
        let p = Path::new(file_path);
        p.with_extension("xmp").to_string_lossy().into_owned()
    };

    // A sidecar wins over the embedded packet; otherwise scan the shared buffer.
    let xmp = if Path::new(&sidecar_path).exists() {
        xmp::parse_sidecar(&sidecar_path)
    } else {
        bytes.and_then(xmp::parse_embedded_from_bytes)
    };

    #[cfg(target_os = "macos")]
    let apple_meta = apple::read_apple_metadata(file_path);
    #[cfg(not(target_os = "macos"))]
    let apple_meta = None;

    let exif_tech = exif.map(|e| e.tech.clone());

    EmbeddedMetadata { xmp, apple: apple_meta, exif_tech }
}
