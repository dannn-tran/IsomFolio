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
    for migration in schema::MIGRATIONS {
        let _ = conn.execute_batch(migration);
    }
    for ddl in schema::ALL_DDL {
        conn.execute_batch(ddl)?;
    }
    Ok(conn)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_asset_file(row: &rusqlite::Row<'_>) -> rusqlite::Result<AssetFile> {
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
    })
}

const FILE_COLS: &str =
    "id, path, filename, folder, extension, size, modified_time, is_orphaned, orphaned_at, created_at_unix, flag";

// ---------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------

pub fn upsert_files(conn: &Connection, files: &[AssetFile]) -> Result<usize, AppError> {
    let mut total = 0;
    for chunk in files.chunks(500) {
        let tx = conn.unchecked_transaction()?;
        for f in chunk {
            conn.execute(
                &format!("INSERT OR REPLACE INTO files ({FILE_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)"),
                params![
                    f.id, f.path, f.name, f.folder, f.ext,
                    f.size_bytes, f.mtime_unix,
                    if f.is_orphaned { 1i32 } else { 0i32 },
                    f.orphaned_at, f.created_at_unix,
                    f.flag as i64,
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

pub fn unmark_orphaned(conn: &Connection, file_id: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE files SET is_orphaned = 0, orphaned_at = NULL WHERE id = ?1",
        [file_id],
    )?;
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

pub fn count_orphans(conn: &Connection) -> Result<usize, AppError> {
    let count: i64 =
        conn.query_row("SELECT COUNT(*) FROM files WHERE is_orphaned = 1", [], |r| r.get(0))?;
    Ok(count as usize)
}

pub fn purge_old_orphans(conn: &Connection, older_than_days: u32) -> Result<usize, AppError> {
    let cutoff = now_unix() - (older_than_days as i64 * 86400);
    let n = conn.execute(
        "DELETE FROM files WHERE is_orphaned = 1 AND orphaned_at IS NOT NULL AND orphaned_at < ?1",
        [cutoff],
    )?;
    Ok(n)
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
    let mut map = std::collections::HashMap::new();
    for row in rows {
        let f = row?;
        map.insert(f.path.clone(), f);
    }
    Ok(map)
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

pub fn upsert_tags(conn: &Connection, file_id: &str, tags: &[String]) -> Result<(), AppError> {
    let tx = conn.unchecked_transaction()?;
    conn.execute("DELETE FROM tags WHERE file_id = ?1", [file_id])?;
    for tag in tags {
        conn.execute(
            "INSERT INTO tags (file_id, tag) VALUES (?1, ?2)",
            params![file_id, tag],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn get_tags_for_file(conn: &Connection, file_id: &str) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare("SELECT tag FROM tags WHERE file_id = ?1 ORDER BY tag")?;
    let rows = stmt.query_map([file_id], |r| r.get::<_, String>(0))?;
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

    let tx = conn.unchecked_transaction()?;
    conn.execute(
        "INSERT OR REPLACE INTO metadata (file_id, rating, label, title, description, creator, subjects, apple_tags) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![file_id, rating, label, title, description, creator_json, subjects_json, apple_json],
    )?;
    tx.commit()?;

    let user_tags = get_tags_for_file(conn, file_id)?;
    let mut fts_tokens: Vec<String> = user_tags;
    fts_tokens.extend(subjects.iter().cloned());
    fts_tokens.extend(apple_tags.iter().cloned());
    if let Some(t) = title {
        fts_tokens.push(t.to_string());
    }
    if let Some(d) = description {
        fts_tokens.push(d.to_string());
    }
    fts_tokens.extend(creator.iter().cloned());
    fts_tokens.dedup();
    fts::update_file_index_tags(conn, file_id, &fts_tokens)?;
    Ok(())
}

pub fn get_metadata(conn: &Connection, file_id: &str) -> Result<Option<EmbeddedMetadata>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT rating, label, title, description, creator, subjects, apple_tags \
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
        ))
    })?;

    let row = match rows.next() {
        Some(r) => r?,
        None => return Ok(None),
    };

    let parse_list = |s: Option<String>| -> Vec<String> {
        s.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default()
    };

    let (rating, label, title, description, creator_json, subjects_json, apple_json) = row;
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

    Ok(Some(EmbeddedMetadata { xmp, apple }))
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
            flag: crate::models::Flag::Unflagged,
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
}
