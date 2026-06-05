use crate::models::FileId;

#[derive(Debug, Clone)]
pub enum FileEvent {
    Created(String),
    Deleted(String),
    Renamed { old_path: String, new_path: String },
    Modified(String),
    SyncProgress(SyncProgress),
    /// A sync has persisted its directory structure (before image indexing) —
    /// the sidebar can reload now so subfolders appear immediately.
    FoldersDiscovered,
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
