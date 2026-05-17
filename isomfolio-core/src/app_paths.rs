use std::path::{Path, PathBuf};

fn app_data_root() -> PathBuf {
    dirs_or_fallback().join("IsomFolio")
}

fn dirs_or_fallback() -> PathBuf {
    // Use standard config dir; fall back to home dir
    if let Some(d) = std::env::var_os("HOME") {
        #[cfg(target_os = "macos")]
        return PathBuf::from(d)
            .join("Library")
            .join("Application Support");
        #[cfg(not(target_os = "macos"))]
        return PathBuf::from(d).join(".config");
    }
    PathBuf::from(".")
}

pub fn db_path(catalog_dir: &str) -> String {
    Path::new(catalog_dir)
        .join("catalog.db")
        .to_string_lossy()
        .into_owned()
}

pub fn thumbnail_cache_dir(catalog_dir: &str) -> String {
    Path::new(catalog_dir)
        .join("thumbnails")
        .to_string_lossy()
        .into_owned()
}

pub fn ensure_directories(catalog_dir: &str) {
    let _ = std::fs::create_dir_all(thumbnail_cache_dir(catalog_dir));
}

pub fn create_catalog(parent_dir: &str, name: &str) -> Result<String, std::io::Error> {
    let catalog_path = Path::new(parent_dir).join(format!("{}.isomfolio", name));
    std::fs::create_dir_all(catalog_path.join("thumbnails"))?;
    Ok(catalog_path.to_string_lossy().into_owned())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Session {
    pub catalog_path: String,
    pub folders: Vec<String>,
}

fn session_file_path() -> PathBuf {
    app_data_root().join("session.json")
}

pub fn read_last_session() -> Option<Session> {
    let path = session_file_path();
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_session(s: &Session) {
    let path = session_file_path();
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    if let Ok(data) = serde_json::to_string(s) {
        let _ = std::fs::write(path, data);
    }
}

fn recent_catalogs_path() -> PathBuf {
    app_data_root().join("recent.txt")
}

pub fn read_recent_catalogs() -> Vec<String> {
    let path = recent_catalogs_path();
    let raw: Vec<String> = if path.exists() {
        std::fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .map(|l| l.to_string())
            .collect()
    } else {
        read_last_session()
            .map(|s| vec![s.catalog_path])
            .unwrap_or_default()
    };
    raw.into_iter()
        .filter(|p| Path::new(p).is_dir())
        .take(5)
        .collect()
}

pub fn save_recent_catalog(catalog_path: &str) {
    let path = recent_catalogs_path();
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let existing: Vec<String> = if path.exists() {
        std::fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .map(|l| l.to_string())
            .collect()
    } else {
        Vec::new()
    };
    let updated: Vec<String> = std::iter::once(catalog_path.to_string())
        .chain(existing.into_iter().filter(|p| p != catalog_path))
        .take(5)
        .collect();
    let _ = std::fs::write(path, updated.join("\n"));
}
