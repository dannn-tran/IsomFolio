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
    let sidecar_path = {
        let p = Path::new(file_path);
        p.with_extension("xmp").to_string_lossy().into_owned()
    };

    let xmp = if Path::new(&sidecar_path).exists() {
        xmp::parse_sidecar(&sidecar_path)
    } else {
        xmp::parse_embedded(file_path)
    };

    #[cfg(target_os = "macos")]
    let apple_meta = apple::read_apple_metadata(file_path);
    #[cfg(not(target_os = "macos"))]
    let apple_meta = None;

    let exif_tech = exif::read_exif(file_path).map(|e| e.tech);

    EmbeddedMetadata { xmp, apple: apple_meta, exif_tech }
}
