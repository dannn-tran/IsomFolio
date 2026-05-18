use std::path::{Path, MAIN_SEPARATOR};

pub fn normalize_path(path: &str) -> String {
    if path.trim().is_empty() {
        return path.to_string();
    }
    let full = std::fs::canonicalize(path)
        .unwrap_or_else(|_| Path::new(path).to_path_buf());
    let s = full
        .to_string_lossy()
        .trim_end_matches(MAIN_SEPARATOR)
        .to_string();
    if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
        s.to_lowercase()
    } else {
        s
    }
}

pub fn same_path(a: &str, b: &str) -> bool {
    a == b
}

pub fn is_descendant_path(ancestor: &str, candidate: &str) -> bool {
    if same_path(ancestor, candidate) {
        return false;
    }
    let prefix = format!("{}{}", ancestor, MAIN_SEPARATOR);
    candidate.starts_with(&prefix)
}

pub fn is_within_subtree(root: &str, path: &str) -> bool {
    same_path(root, path) || is_descendant_path(root, path)
}

pub fn is_under_catalog_dir(path: &str) -> bool {
    Path::new(path)
        .components()
        .any(|c| c.as_os_str().to_string_lossy().ends_with(".isomfolio"))
}

pub fn is_catalog_dir(path: &str) -> bool {
    let p = Path::new(path);
    p.extension().map_or(false, |ext| ext == "isomfolio")
        && p.join("catalog.db").exists()
}

pub fn descendant_like_prefix(root_folder: &str) -> String {
    let trimmed = root_folder.trim_end_matches(MAIN_SEPARATOR);
    format!("{}{}{}", trimmed, MAIN_SEPARATOR, "%")
}
