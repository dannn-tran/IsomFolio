use std::collections::HashMap;

use rusqlite::Connection;

use crate::metadata::EmbeddedMetadata;
use crate::models::*;
use crate::search::query_engine::{execute_manual_album_search, execute_search};
use crate::storage::db;

pub struct Catalog {
    conn: Connection,
}

impl Catalog {
    pub fn open(db_path: &str) -> Result<Self, AppError> {
        let conn = db::open_database(db_path)?;
        Ok(Catalog { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    // Files

    pub fn upsert_files(&self, files: &[AssetFile]) -> Result<usize, AppError> {
        db::upsert_files(&self.conn, files)
    }

    pub fn get_files_by_folder(&self, folder: &str) -> Result<Vec<AssetFile>, AppError> {
        db::get_files_by_folder(&self.conn, folder)
    }

    pub fn get_files_by_folder_recursive(&self, root: &str) -> Result<Vec<AssetFile>, AppError> {
        db::get_files_by_folder_recursive(&self.conn, root)
    }

    pub fn get_file_by_id(&self, file_id: &str) -> Result<Option<AssetFile>, AppError> {
        db::get_file_by_id(&self.conn, file_id)
    }

    pub fn delete_files_by_root_folder(&self, root: &str) -> Result<(), AppError> {
        db::delete_files_by_root_folder(&self.conn, root)
    }

    pub fn mark_orphaned(&self, file_id: &str) -> Result<(), AppError> {
        db::mark_orphaned(&self.conn, file_id)
    }

    pub fn unmark_orphaned(&self, file_id: &str) -> Result<(), AppError> {
        db::unmark_orphaned(&self.conn, file_id)
    }

    pub fn update_file_path(&self, old_path: &str, new_file: &AssetFile) -> Result<(), AppError> {
        db::update_file_path(&self.conn, old_path, new_file)
    }

    pub fn get_folder_counts(&self) -> Result<Vec<(String, usize)>, AppError> {
        db::get_folder_counts(&self.conn)
    }

    pub fn get_all_file_paths_with_mtimes(&self) -> Result<Vec<(String, String, i64)>, AppError> {
        db::get_all_file_paths_with_mtimes(&self.conn)
    }

    pub fn get_indexed_paths_in_folder(
        &self,
        root: &str,
    ) -> Result<HashMap<String, AssetFile>, AppError> {
        db::get_indexed_paths_in_folder(&self.conn, root)
    }

    // Tags

    pub fn upsert_tags(&self, file_id: &str, tags: &[String]) -> Result<(), AppError> {
        db::upsert_tags(&self.conn, file_id, tags)
    }

    pub fn add_tags_merge(&self, file_id: &str, tags: &[String]) -> Result<(), AppError> {
        db::add_tags_merge(&self.conn, file_id, tags)
    }

    pub fn get_tags_for_file(&self, file_id: &str) -> Result<Vec<String>, AppError> {
        db::get_tags_for_file(&self.conn, file_id)
    }

    pub fn get_all_tags(&self) -> Result<Vec<(String, usize)>, AppError> {
        db::get_all_tags(&self.conn)
    }

    pub fn rename_prefixed_tags(&self, old: &str, new: &str) -> Result<usize, AppError> {
        db::rename_prefixed_tags(&self.conn, old, new)
    }

    pub fn delete_tag_with_descendants(&self, tag: &str) -> Result<usize, AppError> {
        db::delete_tag_with_descendants(&self.conn, tag)
    }

    // Metadata

    pub fn upsert_metadata(&self, file_id: &str, meta: &EmbeddedMetadata) -> Result<(), AppError> {
        db::upsert_metadata(&self.conn, file_id, meta)
    }

    pub fn get_metadata(&self, file_id: &str) -> Result<Option<EmbeddedMetadata>, AppError> {
        db::get_metadata(&self.conn, file_id)
    }

    pub fn set_file_rating(&self, file_id: &str, rating: Option<i32>) -> Result<(), AppError> {
        db::set_file_rating(&self.conn, file_id, rating)
    }

    pub fn set_files_rating(&self, file_ids: &[String], rating: Option<i32>) -> Result<(), AppError> {
        db::set_files_rating(&self.conn, file_ids, rating)
    }

    pub fn set_file_flag(&self, file_id: &str, flag: Flag) -> Result<(), AppError> {
        db::set_file_flag(&self.conn, file_id, flag)
    }

    pub fn set_files_flag(&self, file_ids: &[String], flag: Flag) -> Result<(), AppError> {
        db::set_files_flag(&self.conn, file_ids, flag)
    }

    // Albums

    pub fn create_album(&self, album: &Album) -> Result<(), AppError> {
        db::create_album(&self.conn, album)
    }

    pub fn get_all_albums(&self) -> Result<Vec<Album>, AppError> {
        db::get_all_albums(&self.conn)
    }

    pub fn rename_album(&self, album_id: &str, new_name: &str) -> Result<(), AppError> {
        db::rename_album(&self.conn, album_id, new_name)
    }

    pub fn delete_album(&self, album_id: &str) -> Result<(), AppError> {
        db::delete_album(&self.conn, album_id)
    }

    pub fn update_smart_album_query(&self, album_id: &str, query: &SearchQuery) -> Result<(), AppError> {
        db::update_smart_album_query(&self.conn, album_id, query)
    }

    pub fn add_file_to_album(&self, album_id: &str, file_id: &str) -> Result<(), AppError> {
        db::add_file_to_album(&self.conn, album_id, file_id)
    }

    pub fn remove_file_from_album(&self, album_id: &str, file_id: &str) -> Result<(), AppError> {
        db::remove_file_from_album(&self.conn, album_id, file_id)
    }

    pub fn get_all_album_file_counts(&self) -> Result<HashMap<String, usize>, AppError> {
        db::get_all_album_file_counts(&self.conn)
    }

    pub fn copy_album_files(&self, src: &str, dst: &str) -> Result<(), AppError> {
        db::copy_album_files(&self.conn, src, dst)
    }

    // Face clusters

    pub fn save_face_clusters(&self, clusters: &[(String, String, f64, f64, f64, f64)]) -> Result<(), AppError> {
        db::save_face_clusters(&self.conn, clusters)
    }

    pub fn get_face_cluster_summaries(&self) -> Result<Vec<FaceClusterSummary>, AppError> {
        db::get_face_cluster_summaries(&self.conn)
    }

    pub fn get_files_in_face_cluster(&self, cluster_id: &str) -> Result<Vec<AssetFile>, AppError> {
        db::get_files_in_face_cluster(&self.conn, cluster_id)
    }

    pub fn rename_face_cluster(&self, cluster_id: &str, name: &str) -> Result<(), AppError> {
        db::rename_face_cluster(&self.conn, cluster_id, name)
    }

    // Search

    pub fn search(&self, query: &SearchQuery) -> Result<Vec<AssetFile>, AppError> {
        execute_search(&self.conn, query)
    }

    pub fn search_manual_album(&self, album_id: &str, query: &SearchQuery) -> Result<Vec<AssetFile>, AppError> {
        let id = album_id.to_string();
        execute_manual_album_search(&self.conn, &id, query)
    }

    // Scanner support

    pub fn detect_and_store_bursts(&self, folder: &str) -> Result<(), AppError> {
        db::detect_and_store_bursts(&self.conn, folder)
    }
}
