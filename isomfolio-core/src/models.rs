use serde::{Deserialize, Serialize};

pub type FileId = String;
pub type AlbumId = String;

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

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(String),
    #[error("scan error: {0}")]
    Scan(String),
    #[error("thumbnail error for {0}: {1}")]
    Thumbnail(String, String),
    #[error("watcher error: {0}")]
    Watcher(String),
    #[error("metadata error: {0}")]
    Metadata(String),
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Scan(e.to_string())
    }
}
