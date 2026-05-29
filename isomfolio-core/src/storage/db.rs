use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::models::*;
use crate::metadata::EmbeddedMetadata;
use crate::path_utils::descendant_like_prefix;
use crate::search::fts;
use crate::storage::schema;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

pub fn open_database(db_path: &str) -> Result<Connection, AppError> {
    if let Some(parent) = Path::new(db_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Db(e.to_string()))?;
    }
    let conn = Connection::open(db_path)?;
    for pragma in schema::PRAGMAS {
        conn.execute_batch(pragma)?;
    }
    run_migrations(&conn)?;
    for ddl in schema::ALL_DDL {
        conn.execute_batch(ddl)?;
    }
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)",
    )?;
    let current: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let total = schema::MIGRATIONS.len() as i64;
    if current >= total {
        return Ok(());
    }
    for (i, migration) in schema::MIGRATIONS.iter().enumerate() {
        if (i as i64) < current {
            continue;
        }
        let _ = conn.execute_batch(migration);
    }
    conn.execute(
        "DELETE FROM schema_version",
        [],
    )?;
    conn.execute(
        "INSERT INTO schema_version (version) VALUES (?1)",
        [total],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn read_asset_file(row: &rusqlite::Row<'_>) -> rusqlite::Result<AssetFile> {
    use crate::models::Flag;
    Ok(AssetFile {
        id: row.get(0)?,
        path: row.get(1)?,
        name: row.get(2)?,
        folder: row.get(3)?,
        ext: row.get(4)?,
        size_bytes: row.get(5)?,
        mtime_unix: row.get(6)?,
        is_orphaned: row.get::<_, i32>(7)? == 1,
        orphaned_at: row.get(8)?,
        created_at_unix: row.get(9)?,
        flag: Flag::from_i64(row.get::<_, i64>(10).unwrap_or(0)),
        exif_date_unix: row.get(11)?,
        gps_lat: row.get(12)?,
        gps_lon: row.get(13)?,
    })
}

pub const FILE_COLS_BARE: &str =
    "id, path, filename, folder, extension, size, modified_time, is_orphaned, orphaned_at, \
     created_at_unix, flag, exif_date_unix, gps_lat, gps_lon";

pub const FILE_COLS_PREFIXED: &str =
    "f.id, f.path, f.filename, f.folder, f.extension, f.size, f.modified_time, \
     f.is_orphaned, f.orphaned_at, f.created_at_unix, f.flag, \
     f.exif_date_unix, f.gps_lat, f.gps_lon";

const FILE_COLS: &str = FILE_COLS_BARE;

// ---------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------

pub fn upsert_files(conn: &Connection, files: &[AssetFile]) -> Result<usize, AppError> {
    let mut total = 0;
    for chunk in files.chunks(500) {
        let tx = conn.unchecked_transaction()?;
        for f in chunk {
            conn.execute(
                &format!("INSERT OR REPLACE INTO files ({FILE_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)"),
                params![
                    f.id, f.path, f.name, f.folder, f.ext,
                    f.size_bytes, f.mtime_unix,
                    if f.is_orphaned { 1i32 } else { 0i32 },
                    f.orphaned_at, f.created_at_unix,
                    f.flag as i64,
                    f.exif_date_unix, f.gps_lat, f.gps_lon,
                ],
            )?;
            total += 1;
        }
        tx.commit()?;
    }
    Ok(total)
}

pub fn get_files_by_folder(conn: &Connection, folder: &str) -> Result<Vec<AssetFile>, AppError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {FILE_COLS} FROM files WHERE folder = ?1 AND is_orphaned = 0 ORDER BY filename"
    ))?;
    let rows = stmt.query_map([folder], read_asset_file)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_files_by_folder_recursive(
    conn: &Connection,
    root_folder: &str,
) -> Result<Vec<AssetFile>, AppError> {
    let prefix = descendant_like_prefix(root_folder);
    let mut stmt = conn.prepare(&format!(
        "SELECT {FILE_COLS} FROM files WHERE (folder = ?1 OR folder LIKE ?2) AND is_orphaned = 0 ORDER BY filename"
    ))?;
    let rows = stmt.query_map(params![root_folder, prefix], read_asset_file)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_file_by_id(conn: &Connection, file_id: &str) -> Result<Option<AssetFile>, AppError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {FILE_COLS} FROM files WHERE id = ?1"
    ))?;
    let mut rows = stmt.query_map([file_id], read_asset_file)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn delete_files_by_root_folder(conn: &Connection, root_folder: &str) -> Result<(), AppError> {
    let prefix = descendant_like_prefix(root_folder);
    conn.execute(
        "DELETE FROM files WHERE folder = ?1 OR folder LIKE ?2",
        params![root_folder, prefix],
    )?;
    Ok(())
}

pub fn mark_orphaned(conn: &Connection, file_id: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE files SET is_orphaned = 1, orphaned_at = ?1 WHERE id = ?2",
        params![now_unix(), file_id],
    )?;
    Ok(())
}

pub fn mark_orphaned_batch(conn: &Connection, file_ids: &[String]) -> Result<(), AppError> {
    if file_ids.is_empty() {
        return Ok(());
    }
    let ts = now_unix();
    let tx = conn.unchecked_transaction()?;
    for fid in file_ids {
        tx.execute(
            "UPDATE files SET is_orphaned = 1, orphaned_at = ?1 WHERE id = ?2",
            params![ts, fid],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn unmark_orphaned(conn: &Connection, file_id: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE files SET is_orphaned = 0, orphaned_at = NULL WHERE id = ?1",
        [file_id],
    )?;
    Ok(())
}

pub fn count_orphans_in_folder(conn: &Connection, folder: &str) -> Result<usize, AppError> {
    let prefix = crate::path_utils::descendant_like_prefix(folder);
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE is_orphaned = 1 AND (folder = ?1 OR folder LIKE ?2)",
        params![folder, prefix],
        |r| r.get(0),
    )?;
    Ok(n as usize)
}

/// Delete all orphaned catalog records in a folder. Does not touch files on disk.
pub fn purge_orphans_in_folder(conn: &Connection, folder: &str) -> Result<usize, AppError> {
    let prefix = crate::path_utils::descendant_like_prefix(folder);
    let n = conn.execute(
        "DELETE FROM files WHERE is_orphaned = 1 AND (folder = ?1 OR folder LIKE ?2)",
        params![folder, prefix],
    )?;
    Ok(n)
}

/// Relocate a file to a new path, transferring all metadata (tags, rating, flag, album
/// membership, face clusters) from the old path to the new one.
pub fn relocate_file(conn: &Connection, old_id: &str, new_path: &str) -> Result<(), AppError> {
    use std::path::Path;
    let norm = crate::path_utils::normalize_path(new_path);
    let new_id = crate::file_index::compute_file_id(&norm);
    let new_name = Path::new(&norm)
        .file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    let new_folder = Path::new(&norm)
        .parent().and_then(|p| p.to_str()).unwrap_or("").to_string();
    let new_ext = Path::new(&norm)
        .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    let tx = conn.unchecked_transaction()?;
    // Disable FK checks temporarily to allow PK change across tables
    conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;

    // Update child tables before changing the PK
    for table in &["tags", "metadata", "face_clusters", "album_files", "pending_tags"] {
        conn.execute(
            &format!("UPDATE {table} SET file_id = ?1 WHERE file_id = ?2"),
            params![new_id, old_id],
        )?;
    }
    conn.execute(
        "UPDATE files SET id = ?1, path = ?2, filename = ?3, folder = ?4, extension = ?5,
         is_orphaned = 0, orphaned_at = NULL WHERE id = ?6",
        params![new_id, norm, new_name, new_folder, new_ext, old_id],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn update_file_path(
    conn: &Connection,
    old_path: &str,
    new_file: &AssetFile,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE files SET id=?1, path=?2, filename=?3, folder=?4, extension=?5, \
         size=?6, modified_time=?7, created_at_unix=?8 WHERE path=?9",
        params![
            new_file.id, new_file.path, new_file.name, new_file.folder, new_file.ext,
            new_file.size_bytes, new_file.mtime_unix, new_file.created_at_unix,
            old_path,
        ],
    )?;
    Ok(())
}

pub fn delete_file(conn: &Connection, file_id: &str) -> Result<(), AppError> {
    conn.execute("DELETE FROM files WHERE id = ?1", [file_id])?;
    Ok(())
}

pub fn get_folder_counts(conn: &Connection) -> Result<Vec<(String, usize)>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT folder, COUNT(*) FROM files WHERE is_orphaned = 0 GROUP BY folder",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_all_file_ids(conn: &Connection) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare("SELECT id FROM files")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_indexed_paths_in_folder(
    conn: &Connection,
    root_folder: &str,
) -> Result<std::collections::HashMap<String, AssetFile>, AppError> {
    let prefix = descendant_like_prefix(root_folder);
    let mut stmt = conn.prepare(&format!(
        "SELECT {FILE_COLS} FROM files WHERE folder = ?1 OR folder LIKE ?2"
    ))?;
    let rows = stmt.query_map(params![root_folder, prefix], read_asset_file)?;
    rows.map(|r| r.map(|f| (f.path.clone(), f)))
        .collect::<Result<_, _>>()
        .map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

/// Replace the user-owned tag list for a file. Tags not in `tags` are hard-deleted;
/// new tags are inserted if absent (existing rows kept as-is).
pub fn upsert_tags(conn: &Connection, file_id: &str, tags: &[String]) -> Result<(), AppError> {
    let tx = conn.unchecked_transaction()?;
    if tags.is_empty() {
        conn.execute("DELETE FROM tags WHERE file_id = ?1", [file_id])?;
    } else {
        let placeholders: String = (1..=tags.len()).map(|i| format!("?{}", i + 1)).collect::<Vec<_>>().join(",");
        let sql = format!("DELETE FROM tags WHERE file_id = ?1 AND tag NOT IN ({placeholders})");
        let mut stmt = conn.prepare(&sql)?;
        let mut p: Vec<&dyn rusqlite::ToSql> = vec![&file_id];
        for t in tags { p.push(t); }
        stmt.execute(p.as_slice())?;
    }
    for tag in tags {
        conn.execute(
            "INSERT OR IGNORE INTO tags (file_id, tag) VALUES (?1, ?2)",
            params![file_id, tag],
        )?;
    }
    tx.commit()?;
    rebuild_fts_for_file(conn, file_id)?;
    Ok(())
}

/// Add tags without deleting existing ones. Sets confidence for AI-generated tags.
pub fn add_tags_merge(conn: &Connection, file_id: &str, tags: &[String]) -> Result<(), AppError> {
    add_tags_merge_scored(conn, file_id, &tags.iter().map(|t| (t.clone(), None)).collect::<Vec<_>>())
}

pub fn add_tags_merge_scored(conn: &Connection, file_id: &str, tags: &[(String, Option<f32>)]) -> Result<(), AppError> {
    let tx = conn.unchecked_transaction()?;
    for (tag, confidence) in tags {
        conn.execute(
            "INSERT INTO tags (file_id, tag, confidence) VALUES (?1, ?2, ?3)
             ON CONFLICT(file_id, tag) DO UPDATE SET confidence = COALESCE(?3, confidence)",
            params![file_id, tag, confidence],
        )?;
    }
    tx.commit()?;
    rebuild_fts_for_file(conn, file_id)?;
    Ok(())
}

/// Import XMP keywords as tags (additive — never removes existing tags).
pub fn sync_xmp_tags(conn: &Connection, file_id: &str, subjects: &[String]) -> Result<(), AppError> {
    if subjects.is_empty() {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    for tag in subjects {
        conn.execute(
            "INSERT OR IGNORE INTO tags (file_id, tag) VALUES (?1, ?2)",
            params![file_id, tag],
        )?;
    }
    tx.commit()?;
    rebuild_fts_for_file(conn, file_id)?;
    Ok(())
}

/// Import Apple Finder tags (additive — never removes existing tags).
pub fn sync_apple_tags(conn: &Connection, file_id: &str, tags: &[String]) -> Result<(), AppError> {
    sync_xmp_tags(conn, file_id, tags)
}

pub fn insert_pending_tags(conn: &Connection, file_id: &str, tags: &[(String, Option<f32>)]) -> Result<(), AppError> {
    let tx = conn.unchecked_transaction()?;
    for (tag, confidence) in tags {
        conn.execute(
            "INSERT OR IGNORE INTO pending_tags (file_id, tag, confidence) VALUES (?1, ?2, ?3)",
            params![file_id, tag, confidence],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn get_pending_tags(conn: &Connection, file_id: &str) -> Result<Vec<(String, Option<f32>)>, AppError> {
    let mut stmt = conn.prepare("SELECT tag, confidence FROM pending_tags WHERE file_id = ?1 ORDER BY confidence DESC")?;
    let rows = stmt.query_map([file_id], |r| Ok((
        r.get::<_, String>(0)?,
        r.get::<_, Option<f64>>(1)?.map(|v| v as f32),
    )))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_pending_tag_count(conn: &Connection) -> Result<usize, AppError> {
    let n: i64 = conn.query_row("SELECT COUNT(DISTINCT file_id) FROM pending_tags", [], |r| r.get(0))?;
    Ok(n as usize)
}

pub fn accept_pending_tag(conn: &Connection, file_id: &str, tag: &str) -> Result<(), AppError> {
    let conf: Option<f64> = conn
        .query_row(
            "SELECT confidence FROM pending_tags WHERE file_id = ?1 AND tag = ?2",
            params![file_id, tag],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    conn.execute(
        "INSERT INTO tags (file_id, tag, confidence) VALUES (?1, ?2, ?3)
         ON CONFLICT(file_id, tag) DO UPDATE SET confidence = COALESCE(?3, confidence)",
        params![file_id, tag, conf],
    )?;
    conn.execute(
        "DELETE FROM pending_tags WHERE file_id = ?1 AND tag = ?2",
        params![file_id, tag],
    )?;
    rebuild_fts_for_file(conn, file_id)?;
    Ok(())
}

pub fn reject_pending_tag(conn: &Connection, file_id: &str, tag: &str) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM pending_tags WHERE file_id = ?1 AND tag = ?2",
        params![file_id, tag],
    )?;
    Ok(())
}

pub fn accept_all_pending(conn: &Connection, file_id: &str) -> Result<usize, AppError> {
    let pending = get_pending_tags(conn, file_id)?;
    let n = pending.len();
    let tx = conn.unchecked_transaction()?;
    for (tag, conf) in &pending {
        conn.execute(
            "INSERT INTO tags (file_id, tag, confidence) VALUES (?1, ?2, ?3)
             ON CONFLICT(file_id, tag) DO UPDATE SET confidence = COALESCE(?3, confidence)",
            params![file_id, tag, conf],
        )?;
    }
    conn.execute("DELETE FROM pending_tags WHERE file_id = ?1", [file_id])?;
    tx.commit()?;
    rebuild_fts_for_file(conn, file_id)?;
    Ok(n)
}

pub fn reject_all_pending(conn: &Connection, file_id: &str) -> Result<usize, AppError> {
    let n = conn.execute("DELETE FROM pending_tags WHERE file_id = ?1", [file_id])?;
    Ok(n)
}

fn parse_json_strings(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

pub fn rebuild_fts_for_file(conn: &Connection, file_id: &str) -> Result<(), AppError> {
    let mut fts_tokens = get_tags_for_file(conn, file_id)?;
    let mut stmt = conn.prepare(
        "SELECT subjects, apple_tags, title, description, creator FROM metadata WHERE file_id = ?1",
    )?;
    let meta = stmt.query_row([file_id], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    });
    if let Ok((subjects, apple_tags, title, description, creator)) = meta {
        if let Some(ref j) = subjects { fts_tokens.extend(parse_json_strings(j)); }
        if let Some(ref j) = apple_tags { fts_tokens.extend(parse_json_strings(j)); }
        if let Some(t) = title { fts_tokens.push(t); }
        if let Some(d) = description { fts_tokens.push(d); }
        if let Some(ref j) = creator { fts_tokens.extend(parse_json_strings(j)); }
    }
    fts_tokens.dedup();
    fts::update_file_index_tags(conn, file_id, &fts_tokens)?;
    Ok(())
}

pub fn get_tags_for_file(conn: &Connection, file_id: &str) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare("SELECT tag FROM tags WHERE file_id = ?1 ORDER BY tag")?;
    let rows = stmt.query_map([file_id], |r| r.get::<_, String>(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Returns `(tag, confidence)`. Non-null confidence indicates an AI-accepted tag.
pub fn get_tags_with_confidence(conn: &Connection, file_id: &str) -> Result<Vec<(String, Option<f32>)>, AppError> {
    let mut stmt = conn.prepare("SELECT tag, confidence FROM tags WHERE file_id = ?1 ORDER BY tag")?;
    let rows = stmt.query_map([file_id], |r| Ok((
        r.get::<_, String>(0)?,
        r.get::<_, Option<f64>>(1)?.map(|v| v as f32),
    )))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_all_tags(conn: &Connection) -> Result<Vec<(String, usize)>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT tag, COUNT(*) as cnt FROM tags GROUP BY tag ORDER BY cnt DESC, tag",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize)))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn rename_tag(conn: &Connection, old_tag: &str, new_tag: &str) -> Result<usize, AppError> {
    let n = conn.execute(
        "UPDATE tags SET tag = ?1 WHERE tag = ?2",
        params![new_tag, old_tag],
    )?;
    Ok(n)
}

pub fn rename_prefixed_tags(
    conn: &Connection,
    old_prefix: &str,
    new_prefix: &str,
) -> Result<usize, AppError> {
    let tx = conn.unchecked_transaction()?;
    let exact = conn.execute(
        "UPDATE tags SET tag = ?1 WHERE tag = ?2",
        params![new_prefix, old_prefix],
    )?;
    let escaped = old_prefix
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("{escaped}/%");
    let old_len = old_prefix.len() as i64;
    let prefix_count = conn.execute(
        "UPDATE tags SET tag = ?1 || SUBSTR(tag, ?2 + 1) WHERE tag LIKE ?3 ESCAPE '\\'",
        params![new_prefix, old_len, pattern],
    )?;
    tx.commit()?;
    Ok(exact + prefix_count)
}

pub fn delete_tag_with_descendants(conn: &Connection, tag: &str) -> Result<usize, AppError> {
    let tx = conn.unchecked_transaction()?;
    let exact = conn.execute("DELETE FROM tags WHERE tag = ?1", [tag])?;
    let escaped = tag
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("{escaped}/%");
    let prefix_count =
        conn.execute("DELETE FROM tags WHERE tag LIKE ?1 ESCAPE '\\'", [pattern])?;
    tx.commit()?;
    Ok(exact + prefix_count)
}

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

pub fn upsert_metadata(
    conn: &Connection,
    file_id: &str,
    meta: &EmbeddedMetadata,
) -> Result<(), AppError> {
    let xmp_core = meta.xmp.as_ref().map(|x| &x.core);
    let xmp_dc = meta.xmp.as_ref().map(|x| &x.dublin_core);

    let rating: Option<i32> = xmp_core.and_then(|c| c.rating);
    let label: Option<&str> = xmp_core.and_then(|c| c.label.as_deref());
    let title: Option<&str> = xmp_dc.and_then(|d| d.title.as_deref());
    let description: Option<&str> = xmp_dc.and_then(|d| d.description.as_deref());
    let creator = xmp_dc.map(|d| d.creator.clone()).unwrap_or_default();
    let subjects = xmp_dc.map(|d| d.subject.clone()).unwrap_or_default();
    let apple_tags: Vec<String> = meta
        .apple
        .as_ref()
        .map(|a| a.user_tags.iter().map(|t| t.text.clone()).collect())
        .unwrap_or_default();

    let creator_json = serde_json::to_string(&creator).unwrap_or_default();
    let subjects_json = serde_json::to_string(&subjects).unwrap_or_default();
    let apple_json = serde_json::to_string(&apple_tags).unwrap_or_default();

    let tech = meta.exif_tech.as_ref();
    let flash_i: Option<i32> = tech.and_then(|t| t.flash).map(|b| b as i32);

    let tx = conn.unchecked_transaction()?;
    conn.execute(
        "INSERT OR REPLACE INTO metadata \
         (file_id, rating, label, title, description, creator, subjects, apple_tags, \
          camera_make, camera_model, lens_model, focal_length_mm, aperture, shutter_speed, iso, flash) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        params![
            file_id, rating, label, title, description, creator_json, subjects_json, apple_json,
            tech.and_then(|t| t.camera_make.as_deref()),
            tech.and_then(|t| t.camera_model.as_deref()),
            tech.and_then(|t| t.lens_model.as_deref()),
            tech.and_then(|t| t.focal_length_mm),
            tech.and_then(|t| t.aperture),
            tech.and_then(|t| t.shutter_speed.as_deref()),
            tech.and_then(|t| t.iso),
            flash_i,
        ],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn get_metadata(conn: &Connection, file_id: &str) -> Result<Option<EmbeddedMetadata>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT rating, label, title, description, creator, subjects, apple_tags, \
                camera_make, camera_model, lens_model, focal_length_mm, aperture, shutter_speed, iso, flash \
         FROM metadata WHERE file_id = ?1",
    )?;
    let mut rows = stmt.query_map([file_id], |row| {
        Ok((
            row.get::<_, Option<i32>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<String>>(9)?,
            row.get::<_, Option<f64>>(10)?,
            row.get::<_, Option<f64>>(11)?,
            row.get::<_, Option<String>>(12)?,
            row.get::<_, Option<i32>>(13)?,
            row.get::<_, Option<i32>>(14)?,
        ))
    })?;

    let row = match rows.next() {
        Some(r) => r?,
        None => return Ok(None),
    };

    let parse_list = |s: Option<String>| -> Vec<String> {
        s.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default()
    };

    let (rating, label, title, description, creator_json, subjects_json, apple_json,
         camera_make, camera_model, lens_model, focal_length_mm, aperture, shutter_speed, iso, flash_i) = row;
    let creator = parse_list(creator_json);
    let subjects = parse_list(subjects_json);
    let apple_tag_strings = parse_list(apple_json);

    let has_xmp = rating.is_some()
        || label.is_some()
        || title.is_some()
        || description.is_some()
        || !creator.is_empty()
        || !subjects.is_empty();

    let xmp = if has_xmp {
        Some(crate::metadata::XmpMetadata {
            core: crate::metadata::XmpCore {
                rating,
                label,
                ..Default::default()
            },
            dublin_core: crate::metadata::DublinCore {
                title,
                description,
                creator,
                subject: subjects,
                ..Default::default()
            },
        })
    } else {
        None
    };

    let apple = if apple_tag_strings.is_empty() {
        None
    } else {
        Some(crate::metadata::AppleMetadata {
            user_tags: apple_tag_strings
                .into_iter()
                .map(|t| crate::metadata::AppleTag { text: t, color_idx: 0 })
                .collect(),
        })
    };

    let has_tech = camera_make.is_some() || camera_model.is_some() || lens_model.is_some()
        || focal_length_mm.is_some() || aperture.is_some() || shutter_speed.is_some()
        || iso.is_some() || flash_i.is_some();

    let exif_tech = if has_tech {
        Some(crate::models::ExifTechMeta {
            camera_make,
            camera_model,
            lens_model,
            focal_length_mm,
            aperture,
            shutter_speed,
            iso,
            flash: flash_i.map(|n| n != 0),
        })
    } else {
        None
    };

    Ok(Some(EmbeddedMetadata { xmp, apple, exif_tech }))
}

// ---------------------------------------------------------------------------
// Burst detection
// ---------------------------------------------------------------------------

pub fn detect_and_store_bursts(conn: &Connection, folder: &str) -> Result<(), AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(exif_date_unix, modified_time) AS t \
         FROM files WHERE folder = ?1 AND is_orphaned = 0 ORDER BY t",
    )?;
    let rows: Vec<(String, i64)> = stmt
        .query_map([folder], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Db(e.to_string()))?;

    let mut groups: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<(String, i64)> = Vec::new();

    for (id, t) in rows {
        let same_burst = current.last().map_or(true, |(_, last_t)| (t - last_t).abs() <= 3);
        if !same_burst {
            groups.push(current.drain(..).map(|(id, _)| id).collect());
        }
        current.push((id, t));
    }
    if !current.is_empty() {
        groups.push(current.into_iter().map(|(id, _)| id).collect());
    }

    let tx = conn.unchecked_transaction()?;
    for group in &groups {
        let burst_id: Option<String> = if group.len() >= 2 {
            use sha2::{Sha256, Digest};
            let mut h = Sha256::new();
            for id in group { h.update(id.as_bytes()); }
            let hex: String = h.finalize().iter().take(6).map(|b| format!("{b:02x}")).collect();
            Some(hex)
        } else {
            None
        };
        for id in group {
            conn.execute(
                "UPDATE files SET burst_id = ?1 WHERE id = ?2",
                rusqlite::params![burst_id, id],
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Albums
// ---------------------------------------------------------------------------

fn serialize_query(q: &SearchQuery) -> String {
    serde_json::to_string(q).unwrap_or_default()
}

fn deserialize_query(json: &str) -> SearchQuery {
    serde_json::from_str(json).unwrap_or_default()
}

fn read_album(row: &rusqlite::Row<'_>) -> rusqlite::Result<Album> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let kind_str: String = row.get(2)?;
    let query_json: Option<String> = row.get(3)?;
    let sort_order: i32 = row.get(4)?;
    let kind = match kind_str.as_str() {
        "smart" => query_json
            .map(|j| AlbumKind::Smart(deserialize_query(&j)))
            .unwrap_or(AlbumKind::Manual),
        _ => AlbumKind::Manual,
    };
    Ok(Album { id, name, kind, sort_order })
}

pub fn create_album(conn: &Connection, album: &Album) -> Result<(), AppError> {
    let (kind_str, query_json) = match &album.kind {
        AlbumKind::Smart(q) => ("smart", Some(serialize_query(q))),
        AlbumKind::Manual => ("manual", None),
    };
    conn.execute(
        "INSERT INTO albums (id, name, kind, query_json, sort_order) VALUES (?1,?2,?3,?4,?5)",
        params![album.id, album.name, kind_str, query_json, album.sort_order],
    )?;
    Ok(())
}

pub fn get_all_albums(conn: &Connection) -> Result<Vec<Album>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, query_json, sort_order FROM albums ORDER BY sort_order, name",
    )?;
    let rows = stmt.query_map([], read_album)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn rename_album(conn: &Connection, album_id: &str, new_name: &str) -> Result<(), AppError> {
    conn.execute("UPDATE albums SET name = ?1 WHERE id = ?2", params![new_name, album_id])?;
    Ok(())
}

pub fn delete_album(conn: &Connection, album_id: &str) -> Result<(), AppError> {
    conn.execute("DELETE FROM albums WHERE id = ?1", [album_id])?;
    Ok(())
}

pub fn update_smart_album_query(
    conn: &Connection,
    album_id: &str,
    query: &SearchQuery,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE albums SET query_json = ?1 WHERE id = ?2 AND kind = 'smart'",
        params![serialize_query(query), album_id],
    )?;
    Ok(())
}

pub fn add_file_to_album(
    conn: &Connection,
    album_id: &str,
    file_id: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR IGNORE INTO album_files (album_id, file_id, added_at) VALUES (?1,?2,?3)",
        params![album_id, file_id, now_unix()],
    )?;
    Ok(())
}

pub fn remove_file_from_album(
    conn: &Connection,
    album_id: &str,
    file_id: &str,
) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM album_files WHERE album_id = ?1 AND file_id = ?2",
        params![album_id, file_id],
    )?;
    Ok(())
}

pub fn get_all_album_file_counts(conn: &Connection) -> Result<HashMap<String, usize>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT album_id, COUNT(*) FROM album_files GROUP BY album_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((id, count as usize))
    })?;
    rows.collect::<Result<HashMap<_, _>, _>>().map_err(Into::into)
}

pub fn set_file_rating(conn: &Connection, file_id: &str, rating: Option<i32>) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO metadata (file_id, rating) VALUES (?1, ?2) \
         ON CONFLICT(file_id) DO UPDATE SET rating = excluded.rating",
        params![file_id, rating],
    )?;
    Ok(())
}

pub fn set_file_flag(conn: &Connection, file_id: &str, flag: crate::models::Flag) -> Result<(), AppError> {
    conn.execute(
        "UPDATE files SET flag = ?1 WHERE id = ?2",
        params![flag as i64, file_id],
    )?;
    Ok(())
}

pub fn set_files_flag(conn: &Connection, file_ids: &[String], flag: crate::models::Flag) -> Result<(), AppError> {
    let tx = conn.unchecked_transaction()?;
    for id in file_ids {
        conn.execute(
            "UPDATE files SET flag = ?1 WHERE id = ?2",
            params![flag as i64, id.as_str()],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn set_files_rating(conn: &Connection, file_ids: &[String], rating: Option<i32>) -> Result<(), AppError> {
    let tx = conn.unchecked_transaction()?;
    for id in file_ids {
        conn.execute(
            "INSERT INTO metadata (file_id, rating) VALUES (?1, ?2) \
             ON CONFLICT(file_id) DO UPDATE SET rating = excluded.rating",
            params![id.as_str(), rating],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn copy_album_files(
    conn: &Connection,
    src_album_id: &str,
    dst_album_id: &str,
) -> Result<(), AppError> {
    let now = now_unix();
    conn.execute(
        "INSERT OR IGNORE INTO album_files (album_id, file_id, added_at) \
         SELECT ?1, file_id, ?2 FROM album_files WHERE album_id = ?3",
        params![dst_album_id, now, src_album_id],
    )?;
    Ok(())
}

pub fn count_album_files(conn: &Connection, album_id: &str) -> Result<usize, AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM album_files WHERE album_id = ?1",
        [album_id],
        |r| r.get(0),
    )?;
    Ok(n as usize)
}

// ---------------------------------------------------------------------------
// Face clustering
// ---------------------------------------------------------------------------

pub fn save_face_clusters(
    conn: &Connection,
    clusters: &[(String, String, f64, f64, f64, f64)],
) -> Result<(), AppError> {
    let tx = conn.unchecked_transaction()?;

    // Collect old cluster→name mappings before wiping
    let old_names: HashMap<String, String> = {
        let mut stmt = conn.prepare(
            "SELECT cluster_id, name FROM face_cluster_names",
        )?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        rows.into_iter().collect()
    };

    // Collect old cluster→file_ids for matching names to new clusters
    let old_members: HashMap<String, Vec<String>> = {
        let mut stmt = conn.prepare(
            "SELECT cluster_id, file_id FROM face_clusters",
        )?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for (cid, fid) in rows {
            map.entry(cid).or_default().push(fid);
        }
        map
    };

    conn.execute_batch("DELETE FROM face_clusters")?;
    for (cluster_id, file_id, x, y, w, h) in clusters {
        conn.execute(
            "INSERT OR IGNORE INTO face_clusters (cluster_id, file_id, bbox_x, bbox_y, bbox_w, bbox_h)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![cluster_id, file_id, x, y, w, h],
        )?;
    }

    // Build new cluster→file_ids
    let mut new_members: HashMap<&str, Vec<&str>> = HashMap::new();
    for (cluster_id, file_id, _, _, _, _) in clusters {
        new_members.entry(cluster_id.as_str()).or_default().push(file_id.as_str());
    }

    // Re-associate names: for each old named cluster, find the new cluster with max overlap
    conn.execute_batch("DELETE FROM face_cluster_names")?;
    let mut assigned_names: HashMap<String, String> = HashMap::new();
    for (old_cid, name) in &old_names {
        let old_fids: std::collections::HashSet<&str> = old_members
            .get(old_cid)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();
        if old_fids.is_empty() {
            continue;
        }
        let best = new_members.iter().max_by_key(|(new_cid, new_fids)| {
            if assigned_names.contains_key(**new_cid) {
                return 0;
            }
            new_fids.iter().filter(|f| old_fids.contains(**f)).count()
        });
        if let Some((&new_cid, new_fids)) = best {
            let overlap = new_fids.iter().filter(|f| old_fids.contains(**f)).count();
            if overlap > 0 && !assigned_names.contains_key(new_cid) {
                assigned_names.insert(new_cid.to_string(), name.clone());
            }
        }
    }
    for (cid, name) in &assigned_names {
        conn.execute(
            "INSERT OR REPLACE INTO face_cluster_names (cluster_id, name) VALUES (?1, ?2)",
            params![cid, name],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn get_face_cluster_summaries(
    conn: &Connection,
) -> Result<Vec<crate::models::FaceClusterSummary>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT fc.cluster_id, fcn.name, COUNT(DISTINCT fc.file_id)
         FROM face_clusters fc
         LEFT JOIN face_cluster_names fcn ON fc.cluster_id = fcn.cluster_id
         GROUP BY fc.cluster_id
         ORDER BY COUNT(DISTINCT fc.file_id) DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::models::FaceClusterSummary {
            cluster_id: row.get(0)?,
            name: row.get(1)?,
            file_count: row.get::<_, i64>(2)? as usize,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_files_in_face_cluster(
    conn: &Connection,
    cluster_id: &str,
) -> Result<Vec<crate::models::AssetFile>, AppError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT DISTINCT {FILE_COLS_PREFIXED}
         FROM files f
         JOIN face_clusters fc ON fc.file_id = f.id
         WHERE fc.cluster_id = ?1 AND f.is_orphaned = 0
         ORDER BY f.modified_time DESC",
    ))?;
    let rows = stmt.query_map([cluster_id], read_asset_file)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_all_file_paths_with_mtimes(
    conn: &Connection,
) -> Result<Vec<(String, String, i64)>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, path, modified_time FROM files WHERE is_orphaned = 0",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn rename_face_cluster(
    conn: &Connection,
    cluster_id: &str,
    name: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR REPLACE INTO face_cluster_names (cluster_id, name) VALUES (?1, ?2)",
        params![cluster_id, name],
    )?;
    Ok(())
}

pub fn merge_face_clusters(conn: &Connection, target_id: &str, source_id: &str) -> Result<(), AppError> {
    let tx = conn.unchecked_transaction()?;
    conn.execute(
        "UPDATE OR IGNORE face_clusters SET cluster_id = ?1 WHERE cluster_id = ?2",
        params![target_id, source_id],
    )?;
    conn.execute("DELETE FROM face_clusters WHERE cluster_id = ?1", [source_id])?;
    conn.execute("DELETE FROM face_cluster_names WHERE cluster_id = ?1", [source_id])?;
    tx.commit()?;
    Ok(())
}

pub fn get_face_cluster_representatives(conn: &Connection) -> Result<Vec<(String, String, f64, f64, f64, f64)>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT fc.cluster_id, f.path, fc.bbox_x, fc.bbox_y, fc.bbox_w, fc.bbox_h
         FROM face_clusters fc
         JOIN files f ON fc.file_id = f.id AND f.is_orphaned = 0
         GROUP BY fc.cluster_id",
    )?;
    let rows = stmt.query_map([], |r| Ok((
        r.get::<_, String>(0)?,
        r.get::<_, String>(1)?,
        r.get::<_, f64>(2)?,
        r.get::<_, f64>(3)?,
        r.get::<_, f64>(4)?,
        r.get::<_, f64>(5)?,
    )))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn remove_file_from_face_cluster(conn: &Connection, cluster_id: &str, file_id: &str) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM face_clusters WHERE cluster_id = ?1 AND file_id = ?2",
        params![cluster_id, file_id],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn open_temp() -> (Connection, NamedTempFile) {
        let f = NamedTempFile::new().unwrap();
        let conn = open_database(f.path().to_str().unwrap()).unwrap();
        (conn, f)
    }

    fn make_file(id: &str, path: &str) -> AssetFile {
        AssetFile {
            id: id.to_string(),
            path: path.to_string(),
            name: "test.jpg".to_string(),
            folder: "/tmp".to_string(),
            ext: "jpg".to_string(),
            size_bytes: 1024,
            mtime_unix: 1000,
            created_at_unix: 900,
            is_orphaned: false,
            orphaned_at: None,
            flag: Flag::Unflagged,
            exif_date_unix: None,
            gps_lat: None,
            gps_lon: None,
        }
    }

    #[test]
    fn upsert_and_get_file() {
        let (conn, _f) = open_temp();
        let file = make_file("abc123", "/tmp/test.jpg");
        upsert_files(&conn, &[file.clone()]).unwrap();
        let result = get_file_by_id(&conn, "abc123").unwrap();
        assert_eq!(result.unwrap().id, "abc123");
    }

    #[test]
    fn orphan_lifecycle() {
        let (conn, _f) = open_temp();
        let file = make_file("abc", "/tmp/x.jpg");
        upsert_files(&conn, &[file]).unwrap();
        mark_orphaned(&conn, "abc").unwrap();
        let f = get_file_by_id(&conn, "abc").unwrap().unwrap();
        assert!(f.is_orphaned);
        assert!(f.orphaned_at.is_some());
        unmark_orphaned(&conn, "abc").unwrap();
        let f2 = get_file_by_id(&conn, "abc").unwrap().unwrap();
        assert!(!f2.is_orphaned);
        assert!(f2.orphaned_at.is_none());
    }

    #[test]
    fn tag_replace_atomic() {
        let (conn, _f) = open_temp();
        let file = make_file("x", "/tmp/x.jpg");
        upsert_files(&conn, &[file]).unwrap();
        upsert_tags(&conn, "x", &["a".to_string(), "b".to_string()]).unwrap();
        upsert_tags(&conn, "x", &["c".to_string()]).unwrap();
        let tags = get_tags_for_file(&conn, "x").unwrap();
        assert_eq!(tags, vec!["c"]);
    }

    #[test]
    fn album_crud() {
        let (conn, _f) = open_temp();
        let album = Album {
            id: "al1".to_string(),
            name: "Favorites".to_string(),
            kind: AlbumKind::Manual,
            sort_order: 0,
        };
        create_album(&conn, &album).unwrap();
        let albums = get_all_albums(&conn).unwrap();
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].name, "Favorites");
        rename_album(&conn, "al1", "Best").unwrap();
        let albums2 = get_all_albums(&conn).unwrap();
        assert_eq!(albums2[0].name, "Best");
        delete_album(&conn, "al1").unwrap();
        assert!(get_all_albums(&conn).unwrap().is_empty());
    }

    #[test]
    fn set_and_get_flag() {
        let (conn, _f) = open_temp();
        let file = make_file("f1", "/tmp/f1.jpg");
        upsert_files(&conn, &[file]).unwrap();
        set_file_flag(&conn, "f1", crate::models::Flag::Pick).unwrap();
        let result = get_file_by_id(&conn, "f1").unwrap().unwrap();
        assert_eq!(result.flag, crate::models::Flag::Pick);
        set_file_flag(&conn, "f1", crate::models::Flag::Reject).unwrap();
        let result2 = get_file_by_id(&conn, "f1").unwrap().unwrap();
        assert_eq!(result2.flag, crate::models::Flag::Reject);
    }

    #[test]
    fn set_files_flag_batch() {
        let (conn, _f) = open_temp();
        upsert_files(&conn, &[make_file("a", "/tmp/a.jpg"), make_file("b", "/tmp/b.jpg")]).unwrap();
        set_files_flag(&conn, &["a".to_string(), "b".to_string()], crate::models::Flag::Pick).unwrap();
        assert_eq!(get_file_by_id(&conn, "a").unwrap().unwrap().flag, crate::models::Flag::Pick);
        assert_eq!(get_file_by_id(&conn, "b").unwrap().unwrap().flag, crate::models::Flag::Pick);
    }

    #[test]
    fn album_membership() {
        let (conn, _f) = open_temp();
        let file = make_file("f1", "/tmp/f1.jpg");
        upsert_files(&conn, &[file]).unwrap();
        let album = Album {
            id: "a1".to_string(),
            name: "A".to_string(),
            kind: AlbumKind::Manual,
            sort_order: 0,
        };
        create_album(&conn, &album).unwrap();
        add_file_to_album(&conn, "a1", "f1").unwrap();
        assert_eq!(count_album_files(&conn, "a1").unwrap(), 1);
        remove_file_from_album(&conn, "a1", "f1").unwrap();
        assert_eq!(count_album_files(&conn, "a1").unwrap(), 0);
    }

    mod tags {
        use super::*;

        fn setup(conn: &Connection) {
            upsert_files(conn, &[make_file("f1", "/p/a.jpg")]).unwrap();
        }

        #[test]
        fn manual_tags_have_no_confidence() {
            let (conn, _f) = open_temp();
            setup(&conn);
            upsert_tags(&conn, "f1", &["landscape".into()]).unwrap();
            let tags = get_tags_with_confidence(&conn, "f1").unwrap();
            assert_eq!(tags.len(), 1);
            assert_eq!(tags[0].0, "landscape");
            assert_eq!(tags[0].1, None);
        }

        #[test]
        fn ai_tags_carry_confidence() {
            let (conn, _f) = open_temp();
            setup(&conn);
            add_tags_merge_scored(&conn, "f1", &[("portrait".into(), Some(0.85))]).unwrap();
            let tags = get_tags_with_confidence(&conn, "f1").unwrap();
            assert_eq!(tags.len(), 1);
            assert!((tags[0].1.unwrap() - 0.85).abs() < 0.01);
        }

        #[test]
        fn manual_upsert_hard_deletes_unlisted_tags() {
            let (conn, _f) = open_temp();
            setup(&conn);
            upsert_tags(&conn, "f1", &["a".into(), "b".into()]).unwrap();
            upsert_tags(&conn, "f1", &["b".into()]).unwrap();
            let tags = get_tags_for_file(&conn, "f1").unwrap();
            assert_eq!(tags, vec!["b".to_string()]);
        }

        #[test]
        fn ai_rerun_updates_confidence_not_duplicate() {
            let (conn, _f) = open_temp();
            setup(&conn);
            add_tags_merge_scored(&conn, "f1", &[("portrait".into(), Some(0.7))]).unwrap();
            add_tags_merge_scored(&conn, "f1", &[("portrait".into(), Some(0.9))]).unwrap();
            let tags = get_tags_with_confidence(&conn, "f1").unwrap();
            assert_eq!(tags.len(), 1);
            assert!((tags[0].1.unwrap() - 0.9).abs() < 0.01);
        }

        #[test]
        fn xmp_sync_adds_tags() {
            let (conn, _f) = open_temp();
            setup(&conn);
            sync_xmp_tags(&conn, "f1", &["paris".into(), "travel".into()]).unwrap();
            let tags = get_tags_for_file(&conn, "f1").unwrap();
            assert_eq!(tags, vec!["paris".to_string(), "travel".to_string()]);
        }

        #[test]
        fn xmp_sync_is_additive_never_removes() {
            let (conn, _f) = open_temp();
            setup(&conn);
            sync_xmp_tags(&conn, "f1", &["paris".into(), "travel".into()]).unwrap();
            // XMP sidecar updated: "travel" no longer present, "rome" added.
            // Additive sync: travel stays, rome added.
            sync_xmp_tags(&conn, "f1", &["paris".into(), "rome".into()]).unwrap();
            let tags = get_tags_for_file(&conn, "f1").unwrap();
            assert_eq!(tags, vec!["paris".to_string(), "rome".to_string(), "travel".to_string()]);
        }

        #[test]
        fn xmp_sync_preserves_existing_ai_confidence() {
            let (conn, _f) = open_temp();
            setup(&conn);
            add_tags_merge_scored(&conn, "f1", &[("paris".into(), Some(0.8))]).unwrap();
            sync_xmp_tags(&conn, "f1", &["paris".into()]).unwrap();
            let tags = get_tags_with_confidence(&conn, "f1").unwrap();
            assert_eq!(tags.len(), 1);
            assert!((tags[0].1.unwrap() - 0.8).abs() < 0.01);
        }
    }

    mod pending_tags {
        use super::*;

        fn setup(conn: &Connection) {
            upsert_files(conn, &[make_file("f1", "/p/a.jpg")]).unwrap();
        }

        #[test]
        fn insert_and_get() {
            let (conn, _f) = open_temp();
            setup(&conn);
            insert_pending_tags(&conn, "f1", &[
                ("portrait".into(), Some(0.9)),
                ("landscape".into(), Some(0.7)),
            ]).unwrap();
            let pending = get_pending_tags(&conn, "f1").unwrap();
            assert_eq!(pending.len(), 2);
            assert_eq!(pending[0].0, "portrait");
        }

        #[test]
        fn accept_moves_to_tags() {
            let (conn, _f) = open_temp();
            setup(&conn);
            insert_pending_tags(&conn, "f1", &[("portrait".into(), Some(0.9))]).unwrap();
            accept_pending_tag(&conn, "f1", "portrait").unwrap();
            assert!(get_pending_tags(&conn, "f1").unwrap().is_empty());
            let tags = get_tags_with_confidence(&conn, "f1").unwrap();
            assert_eq!(tags.len(), 1);
            assert_eq!(tags[0].0, "portrait");
            assert!((tags[0].1.unwrap() - 0.9).abs() < 0.01);
        }

        #[test]
        fn reject_removes() {
            let (conn, _f) = open_temp();
            setup(&conn);
            insert_pending_tags(&conn, "f1", &[("portrait".into(), Some(0.9))]).unwrap();
            reject_pending_tag(&conn, "f1", "portrait").unwrap();
            assert!(get_pending_tags(&conn, "f1").unwrap().is_empty());
            assert!(get_tags_for_file(&conn, "f1").unwrap().is_empty());
        }

        #[test]
        fn accept_all() {
            let (conn, _f) = open_temp();
            setup(&conn);
            insert_pending_tags(&conn, "f1", &[
                ("a".into(), Some(0.9)),
                ("b".into(), Some(0.8)),
            ]).unwrap();
            let n = accept_all_pending(&conn, "f1").unwrap();
            assert_eq!(n, 2);
            assert!(get_pending_tags(&conn, "f1").unwrap().is_empty());
            assert_eq!(get_tags_for_file(&conn, "f1").unwrap().len(), 2);
        }

        #[test]
        fn count() {
            let (conn, _f) = open_temp();
            setup(&conn);
            assert_eq!(get_pending_tag_count(&conn).unwrap(), 0);
            insert_pending_tags(&conn, "f1", &[("a".into(), None)]).unwrap();
            assert_eq!(get_pending_tag_count(&conn).unwrap(), 1);
        }
    }
}
