use std::path::{Path, PathBuf};

pub fn app_data_root() -> PathBuf {
    directories::ProjectDirs::from("", "", "IsomFolio")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn extensions_dir() -> PathBuf {
    app_data_root().join("extensions")
}

pub fn models_dir() -> PathBuf {
    app_data_root().join("models")
}

pub fn face_crop_dir(catalog_dir: &str) -> String {
    Path::new(catalog_dir)
        .join("face-crops")
        .to_string_lossy()
        .into_owned()
}

pub fn face_crop_path(catalog_dir: &str, cluster_id: &str) -> String {
    Path::new(catalog_dir)
        .join("face-crops")
        .join(format!("{cluster_id}.jpg"))
        .to_string_lossy()
        .into_owned()
}

pub fn crash_reports_dir() -> PathBuf {
    app_data_root().join("crash-reports")
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
    let catalog_path = Path::new(parent_dir).join(format!("{}.{}", name, crate::path_utils::CATALOG_EXT));
    std::fs::create_dir_all(catalog_path.join("thumbnails"))?;
    Ok(catalog_path.to_string_lossy().into_owned())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    /// Maps capability name (e.g. "classify") to preferred extension name.
    pub preferred_extension: std::collections::HashMap<String, String>,
    /// Automatically run face clustering after a sync finds new files.
    #[serde(default = "default_true")]
    pub auto_face_cluster: bool,
    /// Import `dc:subject` keywords from XMP sidecars as tags when first discovering a photo.
    /// `None` = user has not yet made a decision; sync will prompt before proceeding.
    #[serde(default)]
    pub import_xmp_tags: Option<bool>,
    /// Import Apple Finder tags (`kMDItemUserTags`) as tags when first discovering a photo.
    #[serde(default)]
    pub import_apple_tags: Option<bool>,
    /// Auto-advance to the next photo after flagging (Pick/Reject/Unflagged) in loupe mode.
    #[serde(default = "default_true")]
    pub auto_advance_on_flag: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            preferred_extension: std::collections::HashMap::new(),
            auto_face_cluster: true,
            import_xmp_tags: None,
            import_apple_tags: None,
            auto_advance_on_flag: true,
        }
    }
}

fn default_true() -> bool { true }

fn settings_path() -> PathBuf {
    app_data_root().join("settings.json")
}

pub fn read_settings() -> AppSettings {
    let path = settings_path();
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_settings(settings: &AppSettings) {
    let path = settings_path();
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    if let Ok(data) = serde_json::to_string(settings) {
        let _ = std::fs::write(path, data);
    }
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
