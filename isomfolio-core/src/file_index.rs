use sha2::{Digest, Sha256};
use std::fs;
use crate::models::{AssetFile, FileId};
use crate::path_utils::normalize_path;

const SUPPORTED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif"];

pub fn is_supported_extension(ext: &str) -> bool {
    let lower = ext.trim_start_matches('.').to_lowercase();
    SUPPORTED_EXTENSIONS.contains(&lower.as_str())
}

pub fn compute_file_id(absolute_path: &str) -> FileId {
    let mut hasher = Sha256::new();
    hasher.update(absolute_path.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn asset_file_from_path(path: &str) -> Option<AssetFile> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let normalized = normalize_path(path);
    let p = std::path::Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !is_supported_extension(&ext) {
        return None;
    }

    let name = p.file_name()?.to_string_lossy().into_owned();
    let folder = normalize_path(
        p.parent()
            .and_then(|d| d.to_str())
            .unwrap_or(""),
    );

    let mtime_unix = meta
        .modified()
        .ok()
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH).ok()
        })
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let created_at_unix = meta
        .created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    Some(AssetFile {
        id: compute_file_id(&normalized),
        path: normalized,
        name,
        folder,
        ext,
        size_bytes: meta.len() as i64,
        mtime_unix,
        created_at_unix,
        is_orphaned: false,
        orphaned_at: None,
    })
}
