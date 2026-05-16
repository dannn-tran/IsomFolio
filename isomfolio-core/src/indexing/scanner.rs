use rusqlite::Connection;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::file_index::{asset_file_from_path, is_supported_extension};
use crate::indexing::types::{ReconcileResult, ScanProgress, ScanResult};
use crate::metadata;
use crate::models::{AppError, AssetFile};
use crate::path_utils::normalize_path;
use crate::storage::db;

pub struct ScannedFile {
    pub asset: AssetFile,
    pub meta: metadata::EmbeddedMetadata,
}

fn discover_paths(root_path: &str) -> impl Iterator<Item = String> {
    let entries = fs::read_dir(root_path)
        .ok()
        .into_iter()
        .flatten();
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| e.path().to_str().map(|s| s.to_string()))
        .filter(|p| {
            Path::new(p)
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| is_supported_extension(ext))
                .unwrap_or(false)
        })
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

    let mut total = 0usize;
    let mut batch: Vec<ScannedFile> = Vec::with_capacity(500);

    for path in discover_paths(root_path) {
        let asset = match asset_file_from_path(&path) {
            Some(a) => a,
            None => continue,
        };
        let meta = metadata::read_metadata(&path);
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

    Ok(ScanResult { total_count: total })
}

pub fn reconcile_folder(
    conn: &Connection,
    root_path: &str,
) -> Result<ReconcileResult, AppError> {
    let indexed: HashMap<String, AssetFile> = db::get_indexed_paths_in_folder(conn, root_path)?;

    let mut fs_files: HashMap<String, fs::Metadata> = HashMap::new();
    let mut sidecar_files: HashMap<String, (String, u64)> = HashMap::new(); // img_path → (xmp_path, mtime)

    match fs::read_dir(root_path) {
        Ok(entries) => {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = match entry.path().to_str() {
                    Some(p) => p.to_string(),
                    None => continue,
                };
                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
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
        Err(e) => eprintln!("Reconcile: cannot enumerate {root_path} — {e}"),
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
