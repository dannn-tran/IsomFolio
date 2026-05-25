use rusqlite::Connection;
use crate::models::AppError;

const FTS5_SPECIALS: &[char] = &['"', '*', '^', '(', ')', '[', ']', '{', '}', '|', '&', '~', '+', ':'];

pub fn sanitize_fts_query(raw: &str) -> String {
    let replaced: String = raw
        .chars()
        .map(|c| if FTS5_SPECIALS.contains(&c) { ' ' } else { c })
        .collect();
    let trimmed = replaced.trim().to_string();
    if trimmed.is_empty() {
        return String::new();
    }
    if raw.ends_with(' ') {
        trimmed
    } else {
        format!("{trimmed}*")
    }
}

pub fn search_fts5(conn: &Connection, raw_query: &str) -> Result<Vec<String>, AppError> {
    let q = sanitize_fts_query(raw_query);
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT files.id FROM file_index \
         JOIN files ON file_index.rowid = files.rowid \
         WHERE file_index MATCH ?1 AND files.is_orphaned = 0 \
         ORDER BY rank",
    )?;
    let rows = stmt.query_map([&q], |r| r.get::<_, String>(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn update_file_index_tags(
    conn: &Connection,
    file_id: &str,
    tags: &[String],
) -> Result<(), AppError> {
    let tag_str = tags.join(" ");
    conn.execute(
        "UPDATE file_index SET tags = ?1 WHERE rowid = (SELECT rowid FROM files WHERE id = ?2)",
        rusqlite::params![tag_str, file_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use crate::models::AssetFile;
    use tempfile::NamedTempFile;

    fn open_temp() -> (Connection, NamedTempFile) {
        let f = NamedTempFile::new().unwrap();
        let conn = db::open_database(f.path().to_str().unwrap()).unwrap();
        (conn, f)
    }

    fn insert_file(conn: &Connection, id: &str, name: &str, folder: &str) {
        let file = AssetFile {
            id: id.to_string(),
            path: format!("{folder}/{name}"),
            name: name.to_string(),
            folder: folder.to_string(),
            ext: "jpg".to_string(),
            size_bytes: 1,
            mtime_unix: 0,
            created_at_unix: 0,
            is_orphaned: false,
            orphaned_at: None,
            flag: crate::models::Flag::Unflagged,
            exif_date_unix: None,
            gps_lat: None,
            gps_lon: None,
        };
        db::upsert_files(conn, &[file]).unwrap();
    }

    #[test]
    fn sanitize_appends_wildcard() {
        assert_eq!(sanitize_fts_query("hello"), "hello*");
    }

    #[test]
    fn sanitize_no_wildcard_trailing_space() {
        assert_eq!(sanitize_fts_query("hello "), "hello");
    }

    #[test]
    fn sanitize_strips_specials() {
        assert_eq!(sanitize_fts_query("hello*world"), "hello world*");
    }

    #[test]
    fn fts_search_finds_by_filename() {
        let (conn, _f) = open_temp();
        insert_file(&conn, "id1", "vacation.jpg", "/photos");
        let ids = search_fts5(&conn, "vacation").unwrap();
        assert!(ids.contains(&"id1".to_string()));
    }

    #[test]
    fn fts_search_empty_query_returns_empty() {
        let (conn, _f) = open_temp();
        insert_file(&conn, "id1", "photo.jpg", "/photos");
        assert!(search_fts5(&conn, "").unwrap().is_empty());
    }

    #[test]
    fn fts_updates_after_tag_change() {
        let (conn, _f) = open_temp();
        insert_file(&conn, "id1", "photo.jpg", "/photos");
        assert!(search_fts5(&conn, "bodybuilder").unwrap().is_empty());
        db::upsert_tags(&conn, "id1", &["bodybuilder".into()]).unwrap();
        let ids = search_fts5(&conn, "bodybuilder").unwrap();
        assert!(ids.contains(&"id1".to_string()));
    }

    #[test]
    fn fts_updates_after_tag_merge() {
        let (conn, _f) = open_temp();
        insert_file(&conn, "id1", "photo.jpg", "/photos");
        db::add_tags_merge(&conn, "id1", &["arnold".into()]).unwrap();
        let ids = search_fts5(&conn, "arnold").unwrap();
        assert!(ids.contains(&"id1".to_string()));
    }
}
