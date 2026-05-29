use serde::{Deserialize, Serialize};

pub type FileId = String;
pub type AlbumId = String;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i64)]
pub enum Flag {
    Unflagged = 0,
    Pick = 1,
    Reject = -1,
}

impl Flag {
    pub fn from_i64(v: i64) -> Self {
        match v {
            1 => Flag::Pick,
            -1 => Flag::Reject,
            _ => Flag::Unflagged,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExifTechMeta {
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub focal_length_mm: Option<f64>,
    pub aperture: Option<f64>,
    pub shutter_speed: Option<String>,
    pub iso: Option<i32>,
    pub flash: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetFile {
    pub id: FileId,
    pub path: String,
    pub name: String,
    pub folder: String,
    pub ext: String,
    pub size_bytes: i64,
    pub mtime_unix: i64,
    pub created_at_unix: i64,
    pub is_orphaned: bool,
    pub orphaned_at: Option<i64>,
    pub flag: Flag,
    pub exif_date_unix: Option<i64>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThumbnailState {
    NotRequested,
    Pending,
    Ready(String),
    Failed(u32),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SortField {
    Name,
    Date,
    Size,
    Ext,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FlagFilter {
    All,
    Picks,
    Rejects,
    Unflagged,
    NotReject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub folder_path: Option<String>,
    pub folder_recursive: bool,
    pub tags: Vec<String>,
    pub extensions: Vec<String>,
    pub date_from: Option<i64>,
    pub date_to: Option<i64>,
    pub sort_by: SortField,
    pub sort_asc: bool,
    pub flag_filter: FlagFilter,
    pub rating_min: Option<i32>,
    pub has_faces: Option<bool>,
    pub has_location: Option<bool>,
    /// Include orphaned (missing) files in results. False by default so search/filter/albums
    /// never surface missing files. Set to true only when browsing a folder with no active criteria.
    #[serde(default)]
    pub include_orphaned: bool,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: None,
            folder_path: None,
            folder_recursive: false,
            tags: Vec::new(),
            extensions: Vec::new(),
            date_from: None,
            date_to: None,
            sort_by: SortField::Name,
            sort_asc: true,
            flag_filter: FlagFilter::All,
            rating_min: None,
            has_faces: None,
            has_location: None,
            include_orphaned: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlbumKind {
    Manual,
    Smart(SearchQuery),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Album {
    pub id: AlbumId,
    pub name: String,
    pub kind: AlbumKind,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FaceClusterSummary {
    pub cluster_id: String,
    pub name: Option<String>,
    pub file_count: usize,
}

#[derive(Debug, Clone)]
pub struct FaceClusterMember {
    pub cluster_id: String,
    pub file_id: String,
    pub bbox_x: f64,
    pub bbox_y: f64,
    pub bbox_w: f64,
    pub bbox_h: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(String),
    #[error("sync error: {0}")]
    Sync(String),
    #[error("thumbnail error for {0}: {1}")]
    Thumbnail(String, String),
    #[error("watcher error: {0}")]
    Watcher(String),
    #[error("metadata error: {0}")]
    Metadata(String),
    #[error("extension error: {0}")]
    Extension(String),
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Sync(e.to_string())
    }
}
