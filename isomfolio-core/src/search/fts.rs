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

fn run_match(conn: &Connection, q: &str) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT files.id FROM file_index \
         JOIN files ON file_index.rowid = files.rowid \
         WHERE file_index MATCH ?1 AND files.is_orphaned = 0 \
         ORDER BY rank",
    )?;
    let rows = stmt.query_map([q], |r| r.get::<_, String>(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn search_fts5(conn: &Connection, raw_query: &str) -> Result<Vec<String>, AppError> {
    let trimmed = raw_query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // A query with whitespace, quotes, a column filter, or parentheses is treated
    // as a full FTS5 expression — enabling boolean (AND/OR/NOT), phrases, and
    // `col:term` filters (filename/tags/folder/meta). Single barewords keep
    // type-ahead prefix matching. Malformed expressions fall back to the
    // sanitised prefix so casual input never errors.
    let looks_like_expr =
        trimmed.contains(char::is_whitespace) || trimmed.contains(['"', ':', '(', ')']);
    if looks_like_expr {
        if let Ok(ids) = run_match(conn, trimmed) {
            return Ok(ids);
        }
    }

    let q = sanitize_fts_query(raw_query);
    if q.is_empty() {
        return Ok(Vec::new());
    }
    // Even the sanitised form can be a degenerate FTS expression (e.g. a trailing
    // operator); never surface a query-syntax error to the user — just no matches.
    Ok(run_match(conn, &q).unwrap_or_default())
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
    fn fts_search_finds_by_caption_and_creator() {
        let (conn, _f) = open_temp();
        insert_file(&conn, "id1", "DSC001.jpg", "/photos");
        db::set_files_description(&conn, &["id1".into()], Some("Fishing boats leaving the harbour")).unwrap();
        db::set_files_creator(&conn, &["id1".into()], Some("Jane Doe")).unwrap();

        assert!(search_fts5(&conn, "harbour").unwrap().contains(&"id1".to_string()));
        assert!(search_fts5(&conn, "Jane").unwrap().contains(&"id1".to_string()));
    }

    #[test]
    fn fts_search_supports_boolean_and_phrase() {
        let (conn, _f) = open_temp();
        insert_file(&conn, "a", "a.jpg", "/p");
        db::set_files_description(&conn, &["a".into()], Some("fishing boats")).unwrap();
        insert_file(&conn, "b", "b.jpg", "/p");
        db::set_files_description(&conn, &["b".into()], Some("fishing nets")).unwrap();

        // Implicit AND across terms.
        let r = search_fts5(&conn, "fishing boats").unwrap();
        assert!(r.contains(&"a".to_string()) && !r.contains(&"b".to_string()));
        // OR.
        let r = search_fts5(&conn, "boats OR nets").unwrap();
        assert!(r.contains(&"a".to_string()) && r.contains(&"b".to_string()));
        // NOT.
        let r = search_fts5(&conn, "fishing NOT nets").unwrap();
        assert!(r.contains(&"a".to_string()) && !r.contains(&"b".to_string()));
        // Malformed expression falls back to a prefix search instead of erroring.
        assert!(search_fts5(&conn, "fishing AND").is_ok());
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
