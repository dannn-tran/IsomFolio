use rusqlite::Connection;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::file_index::{asset_file_from_path, is_supported_extension};
use crate::indexing::types::{ReconcileResult, ScanProgress, ScanResult};
use crate::metadata;
use crate::models::{AppError, AssetFile};
use crate::path_utils::{normalize_path, CATALOG_EXT};
use crate::storage::db;

pub struct ScannedFile {
    pub asset: AssetFile,
    pub meta: metadata::EmbeddedMetadata,
}

fn discover_paths(root_path: &str) -> Vec<String> {
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

pub fn scan_folder(
    conn: &Connection,
    root_path: &str,
    on_batch: &dyn Fn(&[ScannedFile]),
    on_progress: &dyn Fn(ScanProgress),
) -> Result<ScanResult, AppError> {
    let folder_name = Path::new(root_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(root_path)
        .to_string();

    let indexed = db::get_indexed_paths_in_folder(conn, root_path)?;

    let mut total = 0usize;
    let mut new_file_ids: Vec<String> = Vec::new();
    let mut batch: Vec<ScannedFile> = Vec::with_capacity(500);

    for path in discover_paths(root_path) {
        let asset = match asset_file_from_path(&path) {
            Some(a) => a,
            None => continue,
        };
        let is_new = !indexed.contains_key(&asset.path);
        let meta = metadata::read_metadata(&path);
        if is_new {
            new_file_ids.push(asset.id.clone());
        }
        batch.push(ScannedFile { asset, meta });

        if batch.len() >= 500 {
            let assets: Vec<AssetFile> = batch.iter().map(|s| s.asset.clone()).collect();
            db::upsert_files(conn, &assets)?;
            for sf in &batch {
                db::upsert_metadata(conn, &sf.asset.id, &sf.meta)?;
            }
            total += batch.len();
            on_batch(&batch);
            on_progress(ScanProgress {
                total_found: total,
                inserted: total,
                folder_name: folder_name.clone(),
            });
            batch.clear();
        }
    }

    if !batch.is_empty() {
        let assets: Vec<AssetFile> = batch.iter().map(|s| s.asset.clone()).collect();
        db::upsert_files(conn, &assets)?;
        for sf in &batch {
            db::upsert_metadata(conn, &sf.asset.id, &sf.meta)?;
        }
        total += batch.len();
        on_batch(&batch);
        on_progress(ScanProgress {
            total_found: total,
            inserted: total,
            folder_name: folder_name.clone(),
        });
    }

    if let Err(e) = db::detect_and_store_bursts(conn, root_path) {
        eprintln!("[db] detect_and_store_bursts failed: {e}");
    }
    Ok(ScanResult { total_count: total, new_file_ids })
}

pub fn resync_files(conn: &Connection, paths: &[String]) -> Result<(), AppError> {
    for path in paths {
        let Some(asset) = asset_file_from_path(path) else { continue };
        let meta = metadata::read_metadata(path);
        db::upsert_files(conn, &[asset.clone()])?;
        db::upsert_metadata(conn, &asset.id, &meta)?;
    }
    Ok(())
}

pub fn resync_sidecar_files(conn: &Connection, paths: &[String]) -> Result<(), AppError> {
    for path in paths {
        let Some(asset) = asset_file_from_path(path) else { continue };
        let meta = metadata::read_metadata(path);
        db::upsert_metadata(conn, &asset.id, &meta)?;
    }
    Ok(())
}

pub fn apply_reconcile(conn: &Connection, result: ReconcileResult) -> Result<(), AppError> {
    for orphan_id in result.orphaned {
        db::mark_orphaned(conn, &orphan_id)?;
    }
    resync_files(conn, &result.new_or_modified)?;
    resync_sidecar_files(conn, &result.sidecar_changed)?;
    Ok(())
}

pub fn reconcile_folder(
    conn: &Connection,
    root_path: &str,
) -> Result<ReconcileResult, AppError> {
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
                let resolved = ["jpg", "jpeg", "png", "webp", "gif"].iter().find_map(|e| {
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
