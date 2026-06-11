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
    /// Automatically run face clustering after a sync finds new files.
    #[serde(default = "default_true")]
    pub auto_face_cluster: bool,
    /// Import `dc:subject` keywords from XMP sidecars as tags when first discovering a photo.
    /// `None` = unset, treated as enabled (the default). Forward-only: disabling never
    /// purges tags already imported.
    #[serde(default)]
    pub import_xmp_tags: Option<bool>,
    /// Import Apple Finder tags (`kMDItemUserTags`) as tags when first discovering a photo.
    /// `None` = unset, treated as enabled on macOS. Forward-only: disabling never purges
    /// tags already imported.
    #[serde(default)]
    pub import_apple_tags: Option<bool>,
    /// Auto-advance to the next photo after any cull verdict — flag, rating, or
    /// colour label — in loupe mode. `alias` keeps the pre-rename `auto_advance_on_flag`
    /// key readable so existing settings.json (incl. users who turned it off) survives.
    #[serde(default = "default_true", alias = "auto_advance_on_flag")]
    pub auto_advance_on_cull: bool,
    /// Custom inference-engine base URL. `None` = Auto (managed local engine);
    /// `Some(url)` = connect to a user-hosted engine instead of spawning one.
    #[serde(default)]
    pub inference_custom_url: Option<String>,
    /// Port the managed local engine binds (ignored when a custom URL is set).
    #[serde(default = "default_inference_port")]
    pub inference_port: u16,
    /// DBSCAN cosine-distance radius — lower groups only very similar faces.
    #[serde(default = "default_face_eps")]
    pub face_eps: f32,
    /// Minimum faces required to form a person cluster.
    #[serde(default = "default_face_min_pts")]
    pub face_min_pts: u32,
    /// Automatically stack near-duplicate frames (perceptual hash) after thumbnails exist.
    #[serde(default = "default_true")]
    pub auto_stack: bool,
    /// Max Hamming distance (0–64) for two frames to count as the same shot — lower is stricter.
    #[serde(default = "default_stack_threshold")]
    pub stack_threshold: u32,
    /// Max seconds between consecutive frames of one stack.
    #[serde(default = "default_stack_window")]
    pub stack_window_secs: i64,
    /// Whether the one-time "delete is virtual, files on disk untouched" reassurance
    /// has been shown. Set true after the first soft-delete so it never repeats.
    #[serde(default)]
    pub seen_delete_hint: bool,
    /// Compute whole-image scene embeddings (for "Review Scenes" permissive
    /// grouping) opportunistically after thumbnails exist, like auto_stack.
    #[serde(default = "default_true")]
    pub auto_scene_embed: bool,
    /// DBSCAN cosine-distance radius for scene grouping — higher groups looser
    /// (more reframed/varied shots together); lower keeps scenes tight.
    #[serde(default = "default_scene_eps")]
    pub scene_eps: f32,
    /// Minimum frames required to form a scene group.
    #[serde(default = "default_scene_min_pts")]
    pub scene_min_pts: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_face_cluster: true,
            import_xmp_tags: None,
            import_apple_tags: None,
            auto_advance_on_cull: true,
            inference_custom_url: None,
            inference_port: default_inference_port(),
            face_eps: default_face_eps(),
            face_min_pts: default_face_min_pts(),
            auto_stack: true,
            stack_threshold: default_stack_threshold(),
            stack_window_secs: default_stack_window(),
            seen_delete_hint: false,
            auto_scene_embed: true,
            scene_eps: default_scene_eps(),
            scene_min_pts: default_scene_min_pts(),
        }
    }
}

fn default_true() -> bool { true }
fn default_inference_port() -> u16 { 45876 }
fn default_face_eps() -> f32 { 0.4 }
fn default_face_min_pts() -> u32 { 2 }
fn default_stack_threshold() -> u32 { 8 }
fn default_stack_window() -> i64 { 10 }
// Cosine radius in *whitened* embedding space (build_scene_review whitens the
// view first). 0.2 gives tight, useful scene groups on a real shoot without the
// over-grouping that raw global descriptors suffer; higher groups looser.
fn default_scene_eps() -> f32 { 0.2 }
// DBSCAN core threshold (neighbours, self excluded): 1 lets a two-frame scene
// form (two tries at one shot), matching stacks' ≥2 minimum; group_scenes drops
// singletons. Raise it to require denser clusters.
fn default_scene_min_pts() -> u32 { 1 }

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
    #[serde(default)]
    pub folders: Vec<String>,
    /// Token of the last-selected sidebar item, restored on reopening this catalog.
    #[serde(default)]
    pub last_selected: Option<String>,
}

fn session_file_path() -> PathBuf {
    app_data_root().join("session.json")
}

pub fn read_last_session() -> Option<Session> {
    let path = session_file_path();
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Persist the session off the UI thread. Sends to a single background writer
/// that coalesces bursts (rapid sidebar navigation) and writes only the latest —
/// so a folder/album switch never blocks render on disk I/O, and because one
/// thread does every write they can't race or land out of order.
pub fn save_session(s: &Session) {
    use std::sync::mpsc::{channel, Sender};
    use std::sync::OnceLock;
    static WRITER: OnceLock<Sender<Session>> = OnceLock::new();
    let tx = WRITER.get_or_init(|| {
        let (tx, rx) = channel::<Session>();
        std::thread::spawn(move || {
            while let Ok(mut latest) = rx.recv() {
                // Drain any saves that piled up while we were writing; only the
                // newest selection matters.
                while let Ok(newer) = rx.try_recv() {
                    latest = newer;
                }
                write_session_to_disk(&latest);
            }
        });
        tx
    });
    let _ = tx.send(s.clone());
}

fn write_session_to_disk(s: &Session) {
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
