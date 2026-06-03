use std::path::{Path, MAIN_SEPARATOR};

pub const CATALOG_EXT: &str = "isfcatalog";

/// Case-fold a path *string* on filesystems that are case-insensitive
/// (macOS, Windows). A no-op elsewhere, where case is significant.
pub fn fold_case(s: &str) -> String {
    if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// Canonicalised path *with original casing preserved* — symlinks, `.`/`..` and
/// relative segments resolved, but never case-folded. This is the user-facing
/// form (real on-disk names). Falls back to the input when the path can't be
/// read (offline/missing). Use [`normalize_path`] for the db key/id form.
pub fn display_path(path: &str) -> String {
    if path.trim().is_empty() {
        return path.to_string();
    }
    let full = std::fs::canonicalize(path).unwrap_or_else(|_| Path::new(path).to_path_buf());
    full.to_string_lossy()
        .trim_end_matches(MAIN_SEPARATOR)
        .to_string()
}

/// The db key / id form of a path: canonicalised and case-folded so a file
/// reached via any casing collates to one stable identity. Display names are
/// kept separately (see [`display_path`]); this form is *not* user-facing.
pub fn normalize_path(path: &str) -> String {
    if path.trim().is_empty() {
        return path.to_string();
    }
    fold_case(&display_path(path))
}

pub fn descendant_like_prefix(root_folder: &str) -> String {
    let trimmed = root_folder.trim_end_matches(MAIN_SEPARATOR);
    format!("{}{}{}", trimmed, MAIN_SEPARATOR, "%")
}
