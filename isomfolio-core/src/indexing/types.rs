use crate::models::FileId;

#[derive(Debug, Clone)]
pub enum FileEvent {
    Created(String),
    Deleted(String),
    Renamed { old_path: String, new_path: String },
    Modified(String),
    SyncProgress(SyncProgress),
}

#[derive(Debug, Clone, Default)]
pub struct ReconcileResult {
    pub new_or_modified: Vec<String>,
    pub orphaned: Vec<FileId>,
    pub sidecar_changed: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ThumbnailRequest {
    pub file_id: FileId,
    pub file_path: String,
    pub priority: i32,
}

#[derive(Debug, Clone)]
pub struct SyncProgress {
    pub total_found: usize,
    pub inserted: usize,
    pub folder_name: String,
}

#[derive(Debug, Clone)]
pub struct SyncResult {
    pub total_count: usize,
    pub new_file_ids: Vec<String>,
}
