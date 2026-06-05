use rusqlite::Connection;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::file_index::{asset_file_from_path, is_supported_extension};
use crate::indexing::types::{SyncProgress, SyncResult};
use crate::metadata;
use crate::models::{AppError, AssetFile};
use crate::path_utils::{display_path, normalize_path, CATALOG_EXT};
use crate::storage::db;

#[derive(Clone)]
pub struct ScannedFile {
    pub asset: AssetFile,
    pub meta: metadata::EmbeddedMetadata,
}

fn discover_paths(root_path: &str, recursive: bool) -> Vec<String> {
    let mut results = Vec::new();
    let mut dirs = vec![root_path.to_string()];
    while let Some(dir) = dirs.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let path = match entry.path().to_str() {
                Some(p) => p.to_string(),
                None => continue,
            };
            if ft.is_dir() {
                if !recursive {
                    continue;
                }
                let is_catalog = Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map_or(false, |n| n.ends_with(&format!(".{CATALOG_EXT}")));
                if !is_catalog {
                    dirs.push(path);
                }
            } else if ft.is_file()
                && Path::new(&path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| is_supported_extension(e))
                    .unwrap_or(false)
            {
                results.push(path);
            }
        }
    }
    results
}

/// Enumerate directories under `root_path` (including the root), skipping
/// catalog bundles. Cheap relative to indexing — no file stat or decode — so it
/// can populate the folder tree before image indexing runs.
fn discover_dirs(root_path: &str, recursive: bool) -> Vec<String> {
    let mut results = vec![root_path.to_string()];
    if !recursive {
        return results;
    }
    let mut dirs = vec![root_path.to_string()];
    while let Some(dir) = dirs.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                continue;
            }
            let path = match entry.path().to_str() {
                Some(p) => p.to_string(),
                None => continue,
            };
            let is_catalog = Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| n.ends_with(&format!(".{CATALOG_EXT}")));
            if !is_catalog {
                results.push(path.clone());
                dirs.push(path);
            }
        }
    }
    results
}

/// Sync a folder. For NEW files only, XMP/Apple keywords are imported as tags
/// (gated by `import_xmp_tags` / `import_apple_tags`). Existing files are not
/// re-imported — that's an explicit user gesture via `import_external_tags`.
pub fn sync_folder(
    conn: &Connection,
    root_path: &str,
    on_batch: &dyn Fn(&[ScannedFile]),
    on_progress: &dyn Fn(SyncProgress),
    on_dirs: &dyn Fn(Vec<(String, String)>),
    import_xmp_tags: bool,
    import_apple_tags: bool,
    recursive: bool,
) -> Result<SyncResult, AppError> {
    let folder_name = Path::new(root_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(root_path)
        .to_string();

    // Hand the caller the directory structure up front (cheap dir-only walk) so
    // it can show subfolders immediately, before any image is indexed. The list
    // is session state on the app side — not persisted — so it never drifts from
    // disk; once these folders' files are indexed they come from the catalog.
    let dir_rows: Vec<(String, String)> = discover_dirs(root_path, recursive)
        .into_iter()
        .map(|d| (normalize_path(&d), display_path(&d)))
        .collect();
    if !dir_rows.is_empty() {
        on_dirs(dir_rows);
    }

    // Match the stored (normalised) `folder` column, not the raw picker path —
    // otherwise a mixed-case root finds nothing on case-folded filesystems and
    // both orphan- and burst-detection silently no-op.
    let folder_key = normalize_path(root_path);
    let indexed = db::get_indexed_paths_in_folder(conn, &folder_key)?;
    // Identity is volume-stable, so a file already catalogued (anywhere) is not
    // "new" even if its path changed because the drive remounted — keeps remounts
    // out of the import-batch count and preserves once-imported metadata.
    let existing_ids: HashSet<String> = db::get_all_file_ids(conn)?.into_iter().collect();
    // Paths seen on disk this scan; anything indexed-but-unseen is a deletion.
    let mut seen: HashSet<String> = HashSet::new();

    let mut total = 0usize;
    let mut new_file_ids: Vec<String> = Vec::new();
    let mut batch: Vec<(ScannedFile, bool)> = Vec::with_capacity(500);

    let flush_batch = |conn: &Connection, batch: &[(ScannedFile, bool)]| -> Result<(), AppError> {
        let assets: Vec<AssetFile> = batch.iter().map(|(s, _)| s.asset.clone()).collect();
        db::upsert_files(conn, &assets)?;
        for (sf, is_new) in batch {
            db::upsert_metadata(conn, &sf.asset.id, &sf.meta)?;
            if *is_new {
                if import_xmp_tags {
                    if let Some(ref xmp) = sf.meta.xmp {
                        db::sync_xmp_tags(conn, &sf.asset.id, &xmp.dublin_core.subject)?;
                    }
                }
                if import_apple_tags {
                    if let Some(ref apple) = sf.meta.apple {
                        let names: Vec<String> = apple.user_tags.iter().map(|t| t.text.clone()).collect();
                        db::sync_apple_tags(conn, &sf.asset.id, &names)?;
                    }
                }
            }
        }
        Ok(())
    };

    for path in discover_paths(root_path, recursive) {
        let asset = match asset_file_from_path(&path) {
            Some(a) => a,
            None => continue,
        };
        let is_new = !existing_ids.contains(&asset.id);
        seen.insert(asset.path.clone());
        let meta = metadata::read_metadata(&path);
        if is_new {
            new_file_ids.push(asset.id.clone());
        }
        batch.push((ScannedFile { asset, meta }, is_new));

        if batch.len() >= 500 {
            flush_batch(conn, &batch)?;
            total += batch.len();
            let scanned: Vec<ScannedFile> = batch.iter().map(|(s, _)| s.clone()).collect();
            on_batch(&scanned);
            on_progress(SyncProgress {
                total_found: total,
                inserted: total,
                folder_name: folder_name.clone(),
            });
            batch.clear();
        }
    }

    if !batch.is_empty() {
        flush_batch(conn, &batch)?;
        total += batch.len();
        let scanned: Vec<ScannedFile> = batch.iter().map(|(s, _)| s.clone()).collect();
        on_batch(&scanned);
        on_progress(SyncProgress {
            total_found: total,
            inserted: total,
            folder_name: folder_name.clone(),
        });
    }

    // Genuine deletions → orphan ("Missing"). Skipped when the root is
    // unreachable (offline drive): a transient unmount must never orphan a whole
    // folder. Reappeared files were re-upserted above with is_orphaned = 0, so
    // they un-orphan automatically.
    if Path::new(root_path).is_dir() {
        let orphaned: Vec<String> = indexed
            .iter()
            .filter(|(path, file)| !seen.contains(*path) && !file.is_orphaned)
            .map(|(_, file)| file.id.clone())
            .collect();
        if !orphaned.is_empty() {
            db::mark_orphaned_batch(conn, &orphaned)?;
        }
    }

    if let Err(e) = db::detect_and_store_bursts(conn, &folder_key) {
        eprintln!("[db] detect_and_store_bursts failed: {e}");
    }
    Ok(SyncResult { total_count: total, new_file_ids })
}

/// Re-read file identity after a file modification. Only updates the `files` table
/// (path, filename, folder, size, mtime, EXIF capture date). Never touches the
/// `metadata` table or `tags` — those are catalog-owned once the file is indexed.
pub fn resync_files(conn: &Connection, paths: &[String]) -> Result<(), AppError> {
    for path in paths {
        let Some(asset) = asset_file_from_path(path) else { continue };
        db::upsert_files(conn, &[asset])?;
    }
    Ok(())
}

/// Explicit user-triggered re-read of external metadata for the given files.
/// - `import_xmp`: overwrites XMP-derived metadata fields (rating, label, title,
///   description, creator, subjects) AND adds `dc:subject` keywords as tags (additive).
/// - `import_apple`: overwrites apple_tags JSON in metadata AND adds Apple Finder
///   tags as tags (additive).
/// Always runs regardless of global settings — invoked by right-click actions.
pub fn import_external_metadata(
    conn: &Connection,
    paths: &[String],
    import_xmp: bool,
    import_apple: bool,
) -> Result<(), AppError> {
    for path in paths {
        let Some(asset) = asset_file_from_path(path) else { continue };
        let meta = metadata::read_metadata(path);
        if import_xmp {
            db::update_xmp_metadata(conn, &asset.id, &meta)?;
            if let Some(ref xmp) = meta.xmp {
                db::sync_xmp_tags(conn, &asset.id, &xmp.dublin_core.subject)?;
            }
        }
        if import_apple {
            db::update_apple_metadata(conn, &asset.id, &meta)?;
            if let Some(ref apple) = meta.apple {
                let names: Vec<String> = apple.user_tags.iter().map(|t| t.text.clone()).collect();
                db::sync_apple_tags(conn, &asset.id, &names)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    mod sync_orphans {
        use super::*;
        use crate::indexing::types::SyncProgress;
        use crate::storage::db;

        #[test]
        fn deletion_orphans_then_return_unorphans() {
            let cat = tempfile::TempDir::new().unwrap();
            let conn = db::open_database(cat.path().join("c.db").to_str().unwrap()).unwrap();
            let photos = tempfile::TempDir::new().unwrap();
            let root = photos.path().to_str().unwrap();
            fs::write(photos.path().join("a.jpg"), b"x").unwrap();
            fs::write(photos.path().join("b.jpg"), b"x").unwrap();
            let nb = |_: &[ScannedFile]| {};
            let np = |_: SyncProgress| {};
            let key = normalize_path(root);

            sync_folder(&conn, root, &nb, &np, &|_| {}, false, false, true).unwrap();
            let idx = db::get_indexed_paths_in_folder(&conn, &key).unwrap();
            assert_eq!(idx.len(), 2);
            assert!(idx.values().all(|f| !f.is_orphaned));

            // Delete one on disk and re-sync — it becomes orphaned, the other stays.
            fs::remove_file(photos.path().join("a.jpg")).unwrap();
            sync_folder(&conn, root, &nb, &np, &|_| {}, false, false, true).unwrap();
            let idx = db::get_indexed_paths_in_folder(&conn, &key).unwrap();
            assert!(idx.values().find(|f| f.name == "a.jpg").unwrap().is_orphaned);
            assert!(!idx.values().find(|f| f.name == "b.jpg").unwrap().is_orphaned);

            // Bring it back — re-sync clears the orphan flag.
            fs::write(photos.path().join("a.jpg"), b"x").unwrap();
            sync_folder(&conn, root, &nb, &np, &|_| {}, false, false, true).unwrap();
            let idx = db::get_indexed_paths_in_folder(&conn, &key).unwrap();
            assert!(idx.values().all(|f| !f.is_orphaned));
        }
    }

    mod discover_paths {
        use super::*;

        fn setup_tree() -> tempfile::TempDir {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            fs::write(root.join("top.jpg"), b"x").unwrap();
            fs::create_dir(root.join("sub")).unwrap();
            fs::write(root.join("sub").join("nested.jpg"), b"x").unwrap();
            dir
        }

        #[test]
        fn recursive_includes_subfolder_files() {
            let dir = setup_tree();
            let found = discover_paths(dir.path().to_str().unwrap(), true);
            assert_eq!(found.len(), 2);
            assert!(found.iter().any(|p| p.ends_with("nested.jpg")));
        }

        #[test]
        fn shallow_skips_subfolder_files() {
            let dir = setup_tree();
            let found = discover_paths(dir.path().to_str().unwrap(), false);
            assert_eq!(found.len(), 1);
            assert!(found[0].ends_with("top.jpg"));
        }

        #[test]
        #[cfg(unix)]
        fn symlinked_subfolder_not_followed() {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            fs::write(root.join("real.jpg"), b"x").unwrap();

            // Create a real subfolder with a photo in it.
            fs::create_dir(root.join("real_sub")).unwrap();
            fs::write(root.join("real_sub").join("sub.jpg"), b"x").unwrap();

            // Create a symlink that points back to the root — circular reference.
            std::os::unix::fs::symlink(root, root.join("loop")).unwrap();

            let found = discover_paths(root.to_str().unwrap(), true);
            // real.jpg + real_sub/sub.jpg found; loop/ symlink not followed.
            assert_eq!(found.len(), 2);
            assert!(found.iter().any(|p| p.ends_with("real.jpg")));
            assert!(found.iter().any(|p| p.ends_with("sub.jpg")));
        }
    }
}
