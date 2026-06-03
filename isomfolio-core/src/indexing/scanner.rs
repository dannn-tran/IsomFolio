use rusqlite::Connection;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::file_index::{asset_file_from_path, is_supported_extension};
use crate::indexing::types::{ReconcileResult, SyncProgress, SyncResult};
use crate::metadata;
use crate::models::{AppError, AssetFile};
use crate::path_utils::{normalize_path, CATALOG_EXT};
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

/// Sync a folder. For NEW files only, XMP/Apple keywords are imported as tags
/// (gated by `import_xmp_tags` / `import_apple_tags`). Existing files are not
/// re-imported — that's an explicit user gesture via `import_external_tags`.
pub fn sync_folder(
    conn: &Connection,
    root_path: &str,
    on_batch: &dyn Fn(&[ScannedFile]),
    on_progress: &dyn Fn(SyncProgress),
    import_xmp_tags: bool,
    import_apple_tags: bool,
    recursive: bool,
) -> Result<SyncResult, AppError> {
    let folder_name = Path::new(root_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(root_path)
        .to_string();

    let indexed = db::get_indexed_paths_in_folder(conn, root_path)?;

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
        let is_new = !indexed.contains_key(&asset.path);
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

    if let Err(e) = db::detect_and_store_bursts(conn, root_path) {
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

pub fn reconcile_folder(
    conn: &Connection,
    root_path: &str,
) -> Result<ReconcileResult, AppError> {
    // Guard: an unreachable root (e.g. an unmounted removable drive) must be
    // treated as *offline*, never reconciled. Enumerating it would read zero
    // files and orphan the entire folder — exactly the wrong outcome for a
    // drive that's merely unplugged. Offline state is handled at the app layer.
    if !Path::new(root_path).is_dir() {
        return Ok(ReconcileResult::default());
    }
    let indexed: HashMap<String, AssetFile> = db::get_indexed_paths_in_folder(conn, root_path)?;

    let mut fs_files: HashMap<String, fs::Metadata> = HashMap::new();
    let mut sidecar_files: HashMap<String, (String, u64)> = HashMap::new(); // img_path → (xmp_path, mtime)

    let mut dirs = vec![root_path.to_string()];
    while let Some(dir) = dirs.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Reconcile: cannot enumerate {dir} — {e}");
                continue;
            }
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = match entry.path().to_str() {
                Some(p) => p.to_string(),
                None => continue,
            };
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_dir() {
                dirs.push(path);
                continue;
            }
            if !meta.is_file() {
                continue;
            }
            let ext = Path::new(&path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if ext == "xmp" {
                let base = Path::new(&path).with_extension("");
                let resolved = crate::file_index::SUPPORTED_EXTENSIONS.iter().filter(|e| **e != "xmp").find_map(|e| {
                    let candidate = normalize_path(&format!("{}.{}", base.display(), e));
                    if indexed.contains_key(&candidate) {
                        Some(candidate)
                    } else {
                        None
                    }
                });
                if let Some(img_path) = resolved {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let entry = sidecar_files.entry(img_path).or_insert((path.clone(), 0));
                    if mtime > entry.1 {
                        *entry = (path, mtime);
                    }
                }
            } else if is_supported_extension(&ext) {
                fs_files.insert(normalize_path(&path), meta);
            }
        }
    }

    let new_or_modified: Vec<String> = fs_files
        .iter()
        .filter_map(|(path, meta)| {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let size = meta.len() as i64;
            match indexed.get(path) {
                None => Some(path.clone()),
                Some(existing) => {
                    if existing.mtime_unix != mtime || existing.size_bytes != size {
                        Some(path.clone())
                    } else {
                        None
                    }
                }
            }
        })
        .collect();

    let new_or_modified_set: std::collections::HashSet<&str> =
        new_or_modified.iter().map(|s| s.as_str()).collect();

    let orphaned: Vec<String> = indexed
        .iter()
        .filter_map(|(path, file)| {
            if !fs_files.contains_key(path) && !file.is_orphaned {
                Some(file.id.clone())
            } else {
                None
            }
        })
        .collect();

    let sidecar_changed: Vec<String> = sidecar_files
        .iter()
        .filter_map(|(img_path, (_, sidecar_mtime))| {
            if new_or_modified_set.contains(img_path.as_str()) {
                return None;
            }
            match indexed.get(img_path) {
                None => None,
                Some(existing) => {
                    if *sidecar_mtime as i64 > existing.mtime_unix {
                        Some(img_path.clone())
                    } else {
                        None
                    }
                }
            }
        })
        .collect();

    Ok(ReconcileResult { new_or_modified, orphaned, sidecar_changed })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
    }
}
