use std::collections::HashMap;

use rusqlite::Connection;

use crate::indexing::scanner;
use crate::indexing::types::{SyncProgress, SyncResult};
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

    pub fn mark_orphaned_batch(&self, file_ids: &[String]) -> Result<(), AppError> {
        db::mark_orphaned_batch(&self.conn, file_ids)
    }

    pub fn unmark_orphaned(&self, file_id: &str) -> Result<(), AppError> {
        db::unmark_orphaned(&self.conn, file_id)
    }

    pub fn update_file_path(&self, old_path: &str, new_file: &AssetFile) -> Result<(), AppError> {
        db::update_file_path(&self.conn, old_path, new_file)
    }

    pub fn get_folder_counts(&self) -> Result<Vec<(String, String, usize)>, AppError> {
        db::get_folder_counts(&self.conn)
    }

    /// Build the navigable folder tree for the sidebar.
    ///
    /// Seeded from indexed folders (`get_folder_counts`) and unioned with the
    /// library roots at count 0, so a freshly-added root appears in the tree
    /// immediately — before its scan has indexed any files.
    ///
    /// `extra` carries directories discovered by an in-progress scan but not yet
    /// indexed (held in memory by the app, see `App::discovered_folders`), so
    /// subfolders show the moment a recursive add is acknowledged rather than
    /// after indexing. They're session-only: once their files are indexed the
    /// same folders come from `get_folder_counts`, and `build_tree` merges by
    /// path (accumulating counts onto one node), so there's no duplication.
    pub fn folder_tree(
        &self,
        extra: &[(String, String)],
    ) -> Result<Vec<crate::folder_tree::FolderNode>, AppError> {
        let folders = db::get_folder_counts(&self.conn)?;
        let roots = db::list_library_roots(&self.conn)?;
        Ok(Self::folder_tree_from(folders, &roots, extra))
    }

    /// Build the folder forest from already-fetched counts + roots — so the
    /// sidebar load doesn't re-query `get_folder_counts` / `list_library_roots`
    /// (the caller needs both for the flat folder list and offline detection).
    pub fn folder_tree_from(
        mut folders: Vec<(String, String, usize)>,
        library_roots: &[db::LibraryRoot],
        extra: &[(String, String)],
    ) -> Vec<crate::folder_tree::FolderNode> {
        for (path, display) in extra {
            folders.push((path.clone(), display.clone(), 0));
        }
        let mut root_keys = Vec::new();
        for root in library_roots {
            // Empty roots (no files indexed yet) aren't in get_folder_counts, so
            // add them here. Keyed on the normalised path; the stored display path
            // carries real case. Folders with files already supply their display.
            folders.push((root.path.clone(), root.path_display.clone(), 0));
            root_keys.push(root.path.clone());
        }
        // The forest is anchored at the library roots so breadcrumbs start at the
        // user's added folders, not the filesystem root.
        crate::folder_tree::build_tree(&folders, &root_keys)
    }

    /// Library roots are stored case-folded (so they collate with each file's
    /// folded `folder` — otherwise the root shows as a second, original-case
    /// tree) alongside a case-preserved display path for the sidebar.
    pub fn upsert_library_root(&self, path: &str, recursive: bool) -> Result<(), AppError> {
        db::upsert_library_root(
            &self.conn,
            &crate::path_utils::normalize_path(path),
            &crate::path_utils::display_path(path),
            recursive,
        )
    }

    pub fn remove_library_root(&self, path: &str) -> Result<(), AppError> {
        db::remove_library_root(&self.conn, &crate::path_utils::normalize_path(path))
    }

    pub fn record_import_batch(
        &self,
        source_folder: Option<&str>,
        file_ids: &[String],
    ) -> Result<Option<i64>, AppError> {
        db::record_import_batch(&self.conn, source_folder, file_ids)
    }

    pub fn get_import_batches(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<crate::models::ImportBatch>, AppError> {
        db::get_import_batches(&self.conn, limit)
    }

    pub fn list_library_roots(&self) -> Result<Vec<db::LibraryRoot>, AppError> {
        db::list_library_roots(&self.conn)
    }

    pub fn distinct_camera_models(&self) -> Result<Vec<String>, AppError> {
        db::distinct_camera_models(&self.conn)
    }

    pub fn get_all_file_paths_with_mtimes(&self) -> Result<Vec<(String, String, i64)>, AppError> {
        db::get_all_file_paths_with_mtimes(&self.conn)
    }

    pub fn sweep_face_embeddings(&self) -> Result<(), AppError> {
        db::sweep_face_embeddings(&self.conn)
    }

    pub fn get_uncached_face_file_paths(&self) -> Result<Vec<(String, String, i64)>, AppError> {
        db::get_uncached_face_file_paths(&self.conn)
    }

    pub fn insert_face_embeddings(
        &self,
        file_id: &str,
        mtime: i64,
        faces: &[(f64, f64, f64, f64, Vec<f32>)],
    ) -> Result<(), AppError> {
        db::insert_face_embeddings(&self.conn, file_id, mtime, faces)
    }

    pub fn load_all_face_embeddings(&self) -> Result<Vec<crate::models::FaceEmbeddingRow>, AppError> {
        db::load_all_face_embeddings(&self.conn)
    }

    pub fn save_face_centroids(&self, centroids: &[(String, Vec<f32>)]) -> Result<(), AppError> {
        db::save_face_centroids(&self.conn, centroids)
    }

    pub fn load_face_centroids(&self) -> Result<Vec<(String, Vec<f32>)>, AppError> {
        db::load_face_centroids(&self.conn)
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

    pub fn purge_orphans_in_folder(&self, folder: &str) -> Result<usize, AppError> {
        db::purge_orphans_in_folder(&self.conn, folder)
    }

    pub fn relocate_file(&self, old_id: &str, new_path: &str) -> Result<(), AppError> {
        db::relocate_file(&self.conn, old_id, new_path)
    }

    /// Drop cached thumbnails/previews for files no longer in the catalog.
    pub fn sweep_caches(&self, catalog_dir: &str) -> Result<usize, AppError> {
        crate::indexing::thumbnail::sweep_caches(&self.conn, catalog_dir)
    }

    pub fn count_orphans_in_folder(&self, folder: &str) -> Result<usize, AppError> {
        db::count_orphans_in_folder(&self.conn, folder)
    }

    pub fn get_shared_tags(&self, file_ids: &[String]) -> Result<Vec<String>, AppError> {
        if file_ids.is_empty() {
            return Ok(Vec::new());
        }
        let first = db::get_tags_for_file(&self.conn, &file_ids[0])?;
        if file_ids.len() == 1 {
            return Ok(first);
        }
        let mut shared: Vec<String> = first;
        for fid in &file_ids[1..] {
            let tags = db::get_tags_for_file(&self.conn, fid)?;
            shared.retain(|t| tags.contains(t));
        }
        Ok(shared)
    }

    pub fn add_tag_to_files(&self, file_ids: &[String], tag: &str) -> Result<(), AppError> {
        db::add_tag_to_files_bulk(&self.conn, file_ids, tag)
    }

    pub fn remove_tag_from_files(&self, file_ids: &[String], tag: &str) -> Result<(), AppError> {
        for fid in file_ids {
            let mut tags = db::get_tags_for_file(&self.conn, fid)?;
            tags.retain(|t| t != tag);
            db::upsert_tags(&self.conn, fid, &tags)?;
        }
        Ok(())
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

    pub fn set_files_label(&self, file_ids: &[String], label: Option<&str>) -> Result<(), AppError> {
        db::set_files_label(&self.conn, file_ids, label)
    }

    pub fn get_file_labels(&self, file_ids: &[String]) -> Result<std::collections::HashMap<String, String>, AppError> {
        db::get_file_labels(&self.conn, file_ids)
    }

    pub fn get_ratings_for(&self, file_ids: &[String]) -> Result<std::collections::HashMap<String, i32>, AppError> {
        db::get_ratings_for(&self.conn, file_ids)
    }

    pub fn set_files_title(&self, ids: &[String], value: Option<&str>) -> Result<(), AppError> {
        db::set_files_title(&self.conn, ids, value)
    }

    pub fn set_files_description(&self, ids: &[String], value: Option<&str>) -> Result<(), AppError> {
        db::set_files_description(&self.conn, ids, value)
    }

    pub fn set_files_creator(&self, ids: &[String], value: Option<&str>) -> Result<(), AppError> {
        db::set_files_creator(&self.conn, ids, value)
    }

    pub fn set_files_rights(&self, ids: &[String], value: Option<&str>) -> Result<(), AppError> {
        db::set_files_rights(&self.conn, ids, value)
    }

    pub fn xmp_sidecar_for(&self, file_id: &str, existing: Option<&str>) -> Result<String, AppError> {
        db::xmp_sidecar_for(&self.conn, file_id, existing)
    }

    pub fn export_metadata_csv(&self, file_ids: &[String]) -> Result<String, AppError> {
        db::export_metadata_csv(&self.conn, file_ids)
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

    pub fn delete_files(&self, file_ids: &[String]) -> Result<(), AppError> {
        db::delete_files(&self.conn, file_ids)
    }

    pub fn set_files_deleted(&self, file_ids: &[String], deleted: bool) -> Result<(), AppError> {
        db::set_files_deleted(&self.conn, file_ids, deleted)
    }

    pub fn count_deleted(&self) -> Result<usize, AppError> {
        db::count_deleted(&self.conn)
    }

    pub fn get_burst_sizes_for(&self, file_ids: &[String]) -> Result<std::collections::HashMap<String, usize>, AppError> {
        db::get_burst_sizes_for(&self.conn, file_ids)
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

    pub fn save_face_clusters(&self, members: &[FaceClusterMember]) -> Result<(), AppError> {
        let tuples: Vec<(String, String, f64, f64, f64, f64)> = members
            .iter()
            .map(|m| (m.cluster_id.clone(), m.file_id.clone(), m.bbox_x, m.bbox_y, m.bbox_w, m.bbox_h))
            .collect();
        db::save_face_clusters(&self.conn, &tuples)
    }

    pub fn get_face_cluster_summaries(&self) -> Result<Vec<FaceClusterSummary>, AppError> {
        db::get_face_cluster_summaries(&self.conn)
    }

    pub fn rename_face_cluster(&self, cluster_id: &str, name: &str) -> Result<(), AppError> {
        db::rename_face_cluster(&self.conn, cluster_id, name)
    }

    pub fn get_face_cluster_representatives(&self) -> Result<Vec<(String, String, f64, f64, f64, f64)>, AppError> {
        db::get_face_cluster_representatives(&self.conn)
    }

    pub fn merge_face_clusters(&self, target_id: &str, source_id: &str) -> Result<(), AppError> {
        db::merge_face_clusters(&self.conn, target_id, source_id)
    }

    pub fn remove_file_from_face_cluster(&self, cluster_id: &str, file_id: &str) -> Result<(), AppError> {
        db::remove_file_from_face_cluster(&self.conn, cluster_id, file_id)
    }

    // Search

    pub fn search(&self, query: &SearchQuery) -> Result<Vec<AssetFile>, AppError> {
        execute_search(&self.conn, query)
    }

    pub fn search_manual_album(&self, album_id: &str, query: &SearchQuery) -> Result<Vec<AssetFile>, AppError> {
        let id = album_id.to_string();
        execute_manual_album_search(&self.conn, &id, query)
    }

    // Scanner

    pub fn sync_folder(
        &self,
        root_path: &str,
        on_progress: &dyn Fn(SyncProgress),
        on_dirs: &dyn Fn(Vec<(String, String)>),
        import_xmp_tags: bool,
        import_apple_tags: bool,
        recursive: bool,
    ) -> Result<SyncResult, AppError> {
        scanner::sync_folder(
            &self.conn,
            root_path,
            on_progress,
            on_dirs,
            import_xmp_tags,
            import_apple_tags,
            recursive,
        )
    }

    pub fn resync_files(&self, paths: &[String]) -> Result<(), AppError> {
        scanner::resync_files(&self.conn, paths)
    }

    pub fn import_external_metadata(
        &self,
        paths: &[String],
        import_xmp: bool,
        import_apple: bool,
    ) -> Result<(), AppError> {
        scanner::import_external_metadata(&self.conn, paths, import_xmp, import_apple)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn open_temp() -> (Catalog, NamedTempFile) {
        let f = NamedTempFile::new().unwrap();
        let cat = Catalog::open(f.path().to_str().unwrap()).unwrap();
        (cat, f)
    }

    #[test]
    fn folder_tree_includes_just_added_root_with_no_files() {
        let (cat, _f) = open_temp();
        let sep = std::path::MAIN_SEPARATOR;
        let root = format!("{sep}tmp{sep}newshoot");
        cat.upsert_library_root(&root, true).unwrap();

        let tree = cat.folder_tree(&[]).unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].path, root);
        assert_eq!(tree[0].total_count, 0);
    }

    fn all_paths(nodes: &[crate::folder_tree::FolderNode], out: &mut Vec<String>) {
        for n in nodes {
            for seg in &n.chain {
                out.push(seg.path.clone());
            }
            all_paths(&n.children, out);
        }
    }

    #[test]
    fn folder_tree_unions_extra_discovered_folders() {
        let (cat, _f) = open_temp();
        let sep = std::path::MAIN_SEPARATOR;
        let root = format!("{sep}tmp{sep}shoot");
        cat.upsert_library_root(&root, true).unwrap();
        // Simulate an in-progress scan that found two subfolders (in memory, not
        // yet indexed). They should appear in the tree right away.
        let extra = vec![
            (format!("{root}{sep}a"), format!("{root}{sep}a")),
            (format!("{root}{sep}b"), format!("{root}{sep}b")),
        ];

        let tree = cat.folder_tree(&extra).unwrap();
        let mut paths = Vec::new();
        all_paths(&tree, &mut paths);
        assert!(paths.contains(&format!("{root}{sep}a")), "missing a: {paths:?}");
        assert!(paths.contains(&format!("{root}{sep}b")), "missing b: {paths:?}");
    }
}
