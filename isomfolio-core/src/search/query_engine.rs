use rusqlite::Connection;
use crate::models::{AlbumId, AppError, AssetFile, SearchQuery, SortField};
use crate::search::fts;

fn sort_column(f: SortField) -> &'static str {
    match f {
        SortField::Name => "filename",
        SortField::Date => "modified_time",
        SortField::Size => "size",
        SortField::Ext => "extension",
    }
}

fn read_asset_file(row: &rusqlite::Row<'_>) -> rusqlite::Result<AssetFile> {
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
    })
}

const FILE_COLS: &str =
    "f.id, f.path, f.filename, f.folder, f.extension, f.size, f.modified_time, \
     f.is_orphaned, f.orphaned_at, f.created_at_unix";

pub fn execute_search(conn: &Connection, query: &SearchQuery) -> Result<Vec<AssetFile>, AppError> {
    // FTS candidate set
    let fts_ids: Option<Vec<String>> = match &query.text {
        Some(txt) if !txt.trim().is_empty() => {
            let ids = fts::search_fts5(conn, txt)?;
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            Some(ids)
        }
        _ => None,
    };

    // Build SQL dynamically with boxed params
    let mut sql = format!("SELECT {FILE_COLS} FROM files f");
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut param_idx = 1usize;

    // Tag JOINs
    for (i, tag) in query.tags.iter().enumerate() {
        sql.push_str(&format!(
            " JOIN tags t{i} ON f.id = t{i}.file_id AND t{i}.tag = ?{param_idx}"
        ));
        params.push(Box::new(tag.clone()));
        param_idx += 1;
    }

    sql.push_str(" WHERE 1=1");

    // FTS id filter
    if let Some(ids) = &fts_ids {
        let placeholders: Vec<String> = ids
            .iter()
            .map(|id| {
                let p = format!("?{param_idx}");
                params.push(Box::new(id.clone()));
                param_idx += 1;
                p
            })
            .collect();
        sql.push_str(&format!(" AND f.id IN ({})", placeholders.join(",")));
    }

    // Folder filter
    if let Some(folder_path) = &query.folder_path {
        if !folder_path.trim().is_empty() {
            if query.folder_recursive {
                let prefix = crate::path_utils::descendant_like_prefix(folder_path);
                sql.push_str(&format!(
                    " AND (f.folder = ?{param_idx} OR f.folder LIKE ?{})",
                    param_idx + 1
                ));
                params.push(Box::new(folder_path.clone()));
                params.push(Box::new(prefix));
                param_idx += 2;
            } else {
                sql.push_str(&format!(" AND f.folder = ?{param_idx}"));
                params.push(Box::new(folder_path.clone()));
                param_idx += 1;
            }
        }
    }

    // Extension filter
    if !query.extensions.is_empty() {
        let placeholders: Vec<String> = query
            .extensions
            .iter()
            .map(|ext| {
                let p = format!("?{param_idx}");
                params.push(Box::new(
                    ext.trim_start_matches('.').to_lowercase(),
                ));
                param_idx += 1;
                p
            })
            .collect();
        sql.push_str(&format!(" AND f.extension IN ({})", placeholders.join(",")));
    }

    // Date range filter
    if let Some(from) = query.date_from {
        sql.push_str(&format!(" AND f.modified_time >= ?{param_idx}"));
        params.push(Box::new(from));
        param_idx += 1;
    }
    if let Some(to) = query.date_to {
        sql.push_str(&format!(" AND f.modified_time <= ?{param_idx}"));
        params.push(Box::new(to));
        param_idx += 1;
    }

    let dir = if query.sort_asc { "ASC" } else { "DESC" };
    sql.push_str(&format!(" ORDER BY f.{} {}", sort_column(query.sort_by), dir));

    let _ = param_idx; // suppress unused warning

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), read_asset_file)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn execute_manual_album_search(
    conn: &Connection,
    album_id: &AlbumId,
) -> Result<Vec<AssetFile>, AppError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {FILE_COLS} FROM files f \
         JOIN album_files af ON f.id = af.file_id \
         WHERE af.album_id = ?1 \
         ORDER BY af.added_at ASC"
    ))?;
    let rows = stmt.query_map([album_id], read_asset_file)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Album, AlbumKind};
    use crate::storage::db;
    use tempfile::NamedTempFile;

    fn open_temp() -> (Connection, NamedTempFile) {
        let f = NamedTempFile::new().unwrap();
        let conn = db::open_database(f.path().to_str().unwrap()).unwrap();
        (conn, f)
    }

    fn insert(conn: &Connection, id: &str, name: &str, folder: &str, ext: &str, mtime: i64) {
        db::upsert_files(
            conn,
            &[AssetFile {
                id: id.to_string(),
                path: format!("{folder}/{name}"),
                name: name.to_string(),
                folder: folder.to_string(),
                ext: ext.to_string(),
                size_bytes: 100,
                mtime_unix: mtime,
                created_at_unix: 0,
                is_orphaned: false,
                orphaned_at: None,
            }],
        )
        .unwrap();
    }

    #[test]
    fn folder_filter_exact() {
        let (conn, _f) = open_temp();
        insert(&conn, "a", "a.jpg", "/photos", "jpg", 0);
        insert(&conn, "b", "b.jpg", "/other", "jpg", 0);
        let q = SearchQuery { folder_path: Some("/photos".into()), ..Default::default() };
        let results = execute_search(&conn, &q).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "a");
    }

    #[test]
    fn extension_filter() {
        let (conn, _f) = open_temp();
        insert(&conn, "a", "a.jpg", "/p", "jpg", 0);
        insert(&conn, "b", "b.png", "/p", "png", 0);
        let q = SearchQuery { extensions: vec!["jpg".into()], ..Default::default() };
        let results = execute_search(&conn, &q).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ext, "jpg");
    }

    #[test]
    fn sort_by_date_desc() {
        let (conn, _f) = open_temp();
        insert(&conn, "a", "a.jpg", "/p", "jpg", 100);
        insert(&conn, "b", "b.jpg", "/p", "jpg", 200);
        let q = SearchQuery { sort_by: SortField::Date, sort_asc: false, ..Default::default() };
        let results = execute_search(&conn, &q).unwrap();
        assert_eq!(results[0].id, "b");
    }

    #[test]
    fn manual_album_search() {
        let (conn, _f) = open_temp();
        insert(&conn, "f1", "f1.jpg", "/p", "jpg", 0);
        insert(&conn, "f2", "f2.jpg", "/p", "jpg", 0);
        let album = Album { id: "a1".into(), name: "A".into(), kind: AlbumKind::Manual, sort_order: 0 };
        db::create_album(&conn, &album).unwrap();
        db::add_file_to_album(&conn, "a1", "f1").unwrap();
        let results = execute_manual_album_search(&conn, &"a1".to_string()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "f1");
    }
}
