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
    /// Case-preserved (real on-disk) folder path, for display. `folder` is the
    /// case-folded key; this keeps the user-facing names without re-reading disk.
    pub folder_display: String,
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

/// Which flags pass the filter — an OR-combinable inclusion set (Lightroom-style
/// flag toggles), so culls like "Picks OR Unflagged" are expressible. Each bool
/// means "include this flag." Empty (none) or full (all three) both mean *no
/// filtering*. Subsumes the old "hide rejects" toggle: it's just the selection
/// `{pick, unflagged}`, so there's a single source of truth.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FlagSelection {
    pub pick: bool,
    pub unflagged: bool,
    pub reject: bool,
}

impl FlagSelection {
    fn count(&self) -> usize {
        [self.pick, self.unflagged, self.reject].iter().filter(|b| **b).count()
    }

    /// No effective filter: nothing selected or everything selected.
    pub fn shows_all(&self) -> bool {
        matches!(self.count(), 0 | 3)
    }

    pub fn is_active(&self) -> bool {
        !self.shows_all()
    }

    pub fn allows(&self, flag: Flag) -> bool {
        match flag {
            Flag::Pick => self.pick,
            Flag::Unflagged => self.unflagged,
            Flag::Reject => self.reject,
        }
    }

    pub fn toggled(mut self, flag: Flag) -> Self {
        match flag {
            Flag::Pick => self.pick = !self.pick,
            Flag::Unflagged => self.unflagged = !self.unflagged,
            Flag::Reject => self.reject = !self.reject,
        }
        self
    }

    /// Flag values that pass, as their i64 storage codes.
    pub fn allowed_codes(&self) -> Vec<i64> {
        [Flag::Pick, Flag::Unflagged, Flag::Reject]
            .into_iter()
            .filter(|f| self.allows(*f))
            .map(|f| f as i64)
            .collect()
    }
}

/// Star-rating filter. Culling needs more than "≥ N" — e.g. "unrated only"
/// (the review queue) and "≤ N" / "exactly N". `Unrated` and `AtMost` treat a
/// missing rating as 0.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RatingFilter {
    #[default]
    Any,
    Unrated,
    AtLeast(i32),
    Exactly(i32),
    AtMost(i32),
}

impl RatingFilter {
    pub fn is_active(&self) -> bool {
        !matches!(self, RatingFilter::Any)
    }
}

/// How the include `tags` set combines: `All` = a file must have every tag (AND),
/// `Any` = at least one (OR). Excluded tags (`exclude_tags`) are always a
/// NOT-any: a file is dropped if it has any of them.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TagMatch {
    #[default]
    All,
    Any,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub folder_path: Option<String>,
    pub folder_recursive: bool,
    pub tags: Vec<String>,
    /// How `tags` combine (AND/OR). Defaults to `All` for backward compatibility
    /// with smart albums saved before tag OR/NOT existed.
    #[serde(default)]
    pub tag_match: TagMatch,
    /// Tags whose presence excludes a file (NOT). A file is dropped if it has any
    /// of these (or a descendant, by hierarchy prefix).
    #[serde(default)]
    pub exclude_tags: Vec<String>,
    pub extensions: Vec<String>,
    pub date_from: Option<i64>,
    pub date_to: Option<i64>,
    pub sort_by: SortField,
    pub sort_asc: bool,
    #[serde(default)]
    pub flags: FlagSelection,
    #[serde(default)]
    pub rating: RatingFilter,
    pub has_faces: Option<bool>,
    pub has_location: Option<bool>,
    /// Restrict to files belonging to a face cluster (person), by cluster_id.
    #[serde(default)]
    pub person_cluster: Option<String>,
    /// Restrict to files added to the catalog within the last N days. Stored as a
    /// relative window (not an absolute timestamp) so a saved smart album stays
    /// rolling — the cutoff is computed at query time.
    #[serde(default)]
    pub added_within_days: Option<i64>,
    /// Restrict to files whose EXIF camera model matches exactly.
    #[serde(default)]
    pub camera_model: Option<String>,
    /// Restrict to files with this colour label (XMP `xmp:Label`, e.g. "Red").
    #[serde(default)]
    pub color_label: Option<String>,
    /// Include orphaned (missing) files in results. False by default so search/filter/albums
    /// never surface missing files. Set to true only when browsing a folder with no active criteria.
    #[serde(default)]
    pub include_orphaned: bool,
    /// Show *only* virtually-deleted files (the Deleted view). When false, deleted
    /// files are excluded from all results.
    #[serde(default)]
    pub only_deleted: bool,
    /// Collapse bursts: return only one representative per `burst_id` (the
    /// earliest shot) so a burst occupies a single tile.
    #[serde(default)]
    pub collapse_bursts: bool,
    /// Restrict to files belonging to a specific import batch (discrete sync that
    /// added them). The exact set captured at import time — does not drift.
    #[serde(default)]
    pub import_batch: Option<i64>,
}

/// One import event: a sync that added files to the catalog. Listed in the
/// sidebar so "show me what I just brought in" is a single stable click.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportBatch {
    pub id: i64,
    pub created_at_unix: i64,
    pub source_folder: Option<String>,
    pub count: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: None,
            folder_path: None,
            folder_recursive: false,
            tags: Vec::new(),
            tag_match: TagMatch::default(),
            exclude_tags: Vec::new(),
            extensions: Vec::new(),
            date_from: None,
            date_to: None,
            sort_by: SortField::Name,
            sort_asc: true,
            flags: FlagSelection::default(),
            rating: RatingFilter::default(),
            has_faces: None,
            has_location: None,
            person_cluster: None,
            added_within_days: None,
            camera_model: None,
            color_label: None,
            include_orphaned: false,
            only_deleted: false,
            collapse_bursts: false,
            import_batch: None,
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
    /// Shelf this album is filed under, or `None` when it sits ungrouped at the
    /// top of the Albums list. A shelf is purely an organisational container —
    /// deleting one re-files its albums as ungrouped (never deletes them).
    #[serde(default)]
    pub shelf_id: Option<ShelfId>,
}

pub type ShelfId = String;

/// A named container that holds albums (a "bookshelf" of albums). Shelves only
/// group — they have no membership of their own and no query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shelf {
    pub id: ShelfId,
    pub name: String,
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

/// One detected face: its bounding box and embedding vector, as stored in
/// `face_embeddings`. The host clusters these (DBSCAN) into `face_clusters`.
#[derive(Debug, Clone)]
pub struct FaceEmbeddingRow {
    pub file_id: String,
    pub bbox_x: f64,
    pub bbox_y: f64,
    pub bbox_w: f64,
    pub bbox_h: f64,
    pub vec: Vec<f32>,
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
