use sha2::{Digest, Sha256};
use std::fs;
use crate::models::{AssetFile, FileId};
use crate::path_utils::{display_path, fold_case};

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    // Standard formats
    "jpg", "jpeg", "png", "webp", "gif",
    // RAW formats
    "cr2", "cr3", "crw",        // Canon
    "nef", "nrw",               // Nikon
    "arw",                       // Sony
    "raf",                       // Fujifilm
    "orf",                       // Olympus / OM System
    "rw2",                       // Panasonic
    "pef",                       // Pentax / Ricoh
    "dng",                       // Adobe DNG (universal)
    "srw",                       // Samsung
    "erf",                       // Epson
    "mrw",                       // Minolta / Konica Minolta
];

pub fn is_supported_extension(ext: &str) -> bool {
    let lower = ext.trim_start_matches('.').to_lowercase();
    SUPPORTED_EXTENSIONS.contains(&lower.as_str())
}

pub fn compute_file_id(key: &str) -> FileId {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// The stable identity key for a path: `vol:<uuid>:<rel>` when the path lives on
/// a volume we can identify (so the id survives a remount under a different
/// mount point / drive letter), otherwise the case-folded absolute path (the
/// historical behaviour — boot volume, network shares, unknown filesystems).
/// `display` must be the canonicalised, real-case absolute path.
pub fn identity_key_from_display(display: &str) -> String {
    if let Some(v) = crate::volume::resolve(display) {
        if crate::volume::should_key_volume(&v.mount_point) {
            let rel = crate::volume::relative_to_mount(display, &v.mount_point);
            return format!("vol:{}:{}", v.uuid, fold_case(&rel));
        }
    }
    fold_case(display)
}

/// Stable [`FileId`] for a path, volume-aware. Use this everywhere an id is
/// derived from a path so the same file resolves to the same id across remounts.
pub fn compute_file_id_for_path(path: &str) -> FileId {
    compute_file_id(&identity_key_from_display(&display_path(path)))
}

pub fn asset_file_from_path(path: &str) -> Option<AssetFile> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let p = std::path::Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !is_supported_extension(&ext) {
        return None;
    }

    // Canonicalise once: the display form keeps real casing; `normalized` is its
    // case-folded form (the absolute key used for matching / disk access). The
    // folder is online here (we're scanning it), so casing is accurate.
    let file_display = display_path(path);
    let normalized = fold_case(&file_display);
    let name = p.file_name()?.to_string_lossy().into_owned();
    let parent = p.parent().and_then(|d| d.to_str()).unwrap_or("");
    let folder_display = display_path(parent);
    let folder = fold_case(&folder_display);
    // Identity is volume-aware (survives remounts); the absolute `path`/`folder`
    // remain mount-current and rebind on the next sync via upsert ON CONFLICT(id).
    let id = compute_file_id(&identity_key_from_display(&file_display));

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

    let exif = crate::metadata::exif::read_exif(path);

    Some(AssetFile {
        id,
        path: normalized,
        name,
        folder,
        folder_display,
        ext,
        size_bytes: meta.len() as i64,
        mtime_unix,
        created_at_unix,
        is_orphaned: false,
        orphaned_at: None,
        flag: crate::models::Flag::Unflagged,
        exif_date_unix: exif.as_ref().and_then(|e| e.capture_date),
        gps_lat: exif.as_ref().and_then(|e| e.gps_lat),
        gps_lon: exif.as_ref().and_then(|e| e.gps_lon),
    })
}
