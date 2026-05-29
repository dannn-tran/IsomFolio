use rusqlite::Connection;
use crate::models::{AlbumId, AppError, AssetFile, Flag, FlagFilter, SearchQuery, SortField};
use crate::search::fts;
use crate::storage::db::{read_asset_file, FILE_COLS_PREFIXED as FILE_COLS};

fn sort_column(f: SortField) -> &'static str {
    match f {
        SortField::Name => "f.filename",
        SortField::Date => "COALESCE(f.exif_date_unix, f.modified_time)",
        SortField::Size => "f.size",
        SortField::Ext => "f.extension",
    }
}

fn append_flag_filter(sql: &mut String, params: &mut Vec<Box<dyn rusqlite::ToSql>>, param_idx: &mut usize, flag_filter: FlagFilter) {
    match flag_filter {
        FlagFilter::All => {}
        FlagFilter::Picks => {
            sql.push_str(&format!(" AND f.flag = ?{param_idx}"));
            params.push(Box::new(Flag::Pick as i64));
            *param_idx += 1;
        }
        FlagFilter::Rejects => {
            sql.push_str(&format!(" AND f.flag = ?{param_idx}"));
            params.push(Box::new(Flag::Reject as i64));
            *param_idx += 1;
        }
        FlagFilter::Unflagged => {
            sql.push_str(&format!(" AND f.flag = ?{param_idx}"));
            params.push(Box::new(Flag::Unflagged as i64));
            *param_idx += 1;
        }
        FlagFilter::NotReject => {
            sql.push_str(&format!(" AND f.flag != ?{param_idx}"));
            params.push(Box::new(Flag::Reject as i64));
            *param_idx += 1;
        }
    }
}

fn append_rating_filter(sql: &mut String, params: &mut Vec<Box<dyn rusqlite::ToSql>>, param_idx: &mut usize, rating_min: i32) {
    sql.push_str(&format!(" AND COALESCE(m.rating, 0) >= ?{param_idx}"));
    params.push(Box::new(rating_min));
    *param_idx += 1;
}

fn execute_query_inner(
    conn: &Connection,
    album_id: Option<&AlbumId>,
    query: &SearchQuery,
) -> Result<Vec<AssetFile>, AppError> {
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

    let needs_meta = query.rating_min.is_some();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut param_idx = 1usize;

    let mut sql = match album_id {
        Some(_) => format!("SELECT {FILE_COLS} FROM files f JOIN album_files af ON f.id = af.file_id"),
        None => format!("SELECT {FILE_COLS} FROM files f"),
    };

    for (i, tag) in query.tags.iter().enumerate() {
        let like_param = param_idx + 1;
        sql.push_str(&format!(
            " JOIN tags t{i} ON f.id = t{i}.file_id AND (t{i}.tag = ?{param_idx} OR t{i}.tag LIKE ?{like_param} ESCAPE '\\')"
        ));
        params.push(Box::new(tag.clone()));
        let escaped = tag
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        params.push(Box::new(format!("{escaped}/%")));
        param_idx += 2;
    }

    if needs_meta {
        sql.push_str(" LEFT JOIN metadata m ON f.id = m.file_id");
    }

    if let Some(aid) = album_id {
        sql.push_str(&format!(" WHERE af.album_id = ?{param_idx} AND f.is_orphaned = 0"));
        params.push(Box::new(aid.clone()));
        param_idx += 1;
    } else if query.include_orphaned {
        sql.push_str(" WHERE 1=1");
    } else {
        sql.push_str(" WHERE f.is_orphaned = 0");
    }

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

    if let (None, Some(folder_path)) = (
        album_id,
        query.folder_path.as_deref().filter(|p| !p.trim().is_empty()),
    ) {
        if query.folder_recursive {
            let prefix = crate::path_utils::descendant_like_prefix(folder_path);
            sql.push_str(&format!(
                " AND (f.folder = ?{param_idx} OR f.folder LIKE ?{})",
                param_idx + 1
            ));
            params.push(Box::new(folder_path.to_string()));
            params.push(Box::new(prefix));
            param_idx += 2;
        } else {
            sql.push_str(&format!(" AND f.folder = ?{param_idx}"));
            params.push(Box::new(folder_path.to_string()));
            param_idx += 1;
        }
    }

    if !query.extensions.is_empty() {
        let placeholders: Vec<String> = query
            .extensions
            .iter()
            .map(|ext| {
                let p = format!("?{param_idx}");
                params.push(Box::new(ext.trim_start_matches('.').to_lowercase()));
                param_idx += 1;
                p
            })
            .collect();
        sql.push_str(&format!(" AND f.extension IN ({})", placeholders.join(",")));
    }

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

    append_flag_filter(&mut sql, &mut params, &mut param_idx, query.flag_filter);

    if let Some(min) = query.rating_min {
        append_rating_filter(&mut sql, &mut params, &mut param_idx, min);
    }

    if let Some(has_faces) = query.has_faces {
        if has_faces {
            sql.push_str(" AND EXISTS (SELECT 1 FROM face_clusters fc WHERE fc.file_id = f.id)");
        } else {
            sql.push_str(" AND NOT EXISTS (SELECT 1 FROM face_clusters fc WHERE fc.file_id = f.id)");
        }
    }

    if let Some(has_location) = query.has_location {
        if has_location {
            sql.push_str(" AND f.gps_lat IS NOT NULL");
        } else {
            sql.push_str(" AND f.gps_lat IS NULL");
        }
    }

    let dir = if query.sort_asc { "ASC" } else { "DESC" };
    sql.push_str(&format!(" ORDER BY {} {}", sort_column(query.sort_by), dir));

    let _ = param_idx;

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), read_asset_file)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn execute_search(conn: &Connection, query: &SearchQuery) -> Result<Vec<AssetFile>, AppError> {
    execute_query_inner(conn, None, query)
}

pub fn execute_manual_album_search(
    conn: &Connection,
    album_id: &AlbumId,
    query: &SearchQuery,
) -> Result<Vec<AssetFile>, AppError> {
    execute_query_inner(conn, Some(album_id), query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
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
                flag: Flag::Unflagged,
                exif_date_unix: None,
                gps_lat: None,
                gps_lon: None,
            }],
        )
        .unwrap();
    }

    mod flag_filter {
        use super::*;
        use rusqlite::Connection;
        use crate::models::Flag;
        use crate::storage::db;
        use tempfile::NamedTempFile;

        fn setup() -> (Connection, NamedTempFile) {
            let (conn, f) = open_temp();
            insert(&conn, "pick1", "pick1.jpg", "/p", "jpg", 1);
            insert(&conn, "pick2", "pick2.jpg", "/p", "jpg", 2);
            insert(&conn, "reject1", "reject1.jpg", "/p", "jpg", 3);
            insert(&conn, "plain1", "plain1.jpg", "/p", "jpg", 4);
            db::set_file_flag(&conn, "pick1", Flag::Pick).unwrap();
            db::set_file_flag(&conn, "pick2", Flag::Pick).unwrap();
            db::set_file_flag(&conn, "reject1", Flag::Reject).unwrap();
            (conn, f)
        }

        #[test]
        fn filter_picks() {
            let (conn, _f) = setup();
            let q = SearchQuery { flag_filter: FlagFilter::Picks, ..Default::default() };
            let results = execute_search(&conn, &q).unwrap();
            assert_eq!(results.len(), 2);
            assert!(results.iter().all(|r| r.flag == Flag::Pick));
        }

        #[test]
        fn filter_rejects() {
            let (conn, _f) = setup();
            let q = SearchQuery { flag_filter: FlagFilter::Rejects, ..Default::default() };
            let results = execute_search(&conn, &q).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].id, "reject1");
        }

        #[test]
        fn filter_not_reject() {
            let (conn, _f) = setup();
            let q = SearchQuery { flag_filter: FlagFilter::NotReject, ..Default::default() };
            let results = execute_search(&conn, &q).unwrap();
            assert_eq!(results.len(), 3);
            assert!(results.iter().all(|r| r.flag != Flag::Reject));
        }

        #[test]
        fn filter_unflagged() {
            let (conn, _f) = setup();
            let q = SearchQuery { flag_filter: FlagFilter::Unflagged, ..Default::default() };
            let results = execute_search(&conn, &q).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].id, "plain1");
        }
    }

    mod tag_hierarchy {
        use super::*;
        use crate::storage::db;

        #[test]
        fn exact_match() {
            let (conn, _f) = open_temp();
            insert(&conn, "a", "a.jpg", "/p", "jpg", 0);
            insert(&conn, "b", "b.jpg", "/p", "jpg", 0);
            db::upsert_tags(&conn, "a", &["Subject/Arnold".into()]).unwrap();
            db::upsert_tags(&conn, "b", &["Subject/Ronnie".into()]).unwrap();
            let q = SearchQuery { tags: vec!["Subject/Arnold".into()], ..Default::default() };
            let results = execute_search(&conn, &q).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].id, "a");
        }

        #[test]
        fn parent_matches_descendants() {
            let (conn, _f) = open_temp();
            insert(&conn, "a", "a.jpg", "/p", "jpg", 0);
            insert(&conn, "b", "b.jpg", "/p", "jpg", 0);
            insert(&conn, "c", "c.jpg", "/p", "jpg", 0);
            db::upsert_tags(&conn, "a", &["Subject/Arnold".into()]).unwrap();
            db::upsert_tags(&conn, "b", &["Subject/Ronnie".into()]).unwrap();
            db::upsert_tags(&conn, "c", &["Photographer/Art".into()]).unwrap();
            let q = SearchQuery { tags: vec!["Subject".into()], ..Default::default() };
            let results = execute_search(&conn, &q).unwrap();
            assert_eq!(results.len(), 2);
            let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
            assert!(ids.contains(&"a"));
            assert!(ids.contains(&"b"));
        }

        #[test]
        fn no_false_prefix_match() {
            let (conn, _f) = open_temp();
            insert(&conn, "a", "a.jpg", "/p", "jpg", 0);
            db::upsert_tags(&conn, "a", &["Subject Extra".into()]).unwrap();
            let q = SearchQuery { tags: vec!["Subject".into()], ..Default::default() };
            let results = execute_search(&conn, &q).unwrap();
            assert_eq!(results.len(), 0, "'Subject Extra' must not match tag filter 'Subject'");
        }
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

    mod manual_album {
        use super::*;
        use rusqlite::Connection;
        use crate::models::{Album, AlbumKind};
        use crate::storage::db;
        use tempfile::NamedTempFile;

        fn setup() -> (Connection, NamedTempFile) {
            let (conn, f) = open_temp();
            insert(&conn, "f1", "alpha.jpg", "/p", "jpg", 100);
            insert(&conn, "f2", "beta.png", "/p", "png", 200);
            insert(&conn, "f3", "gamma.jpg", "/p", "jpg", 300);
            let album = Album {
                id: "a1".into(),
                name: "A".into(),
                kind: AlbumKind::Manual,
                sort_order: 0,
            };
            db::create_album(&conn, &album).unwrap();
            db::add_file_to_album(&conn, "a1", "f1").unwrap();
            db::add_file_to_album(&conn, "a1", "f2").unwrap();
            (conn, f)
        }

        #[test]
        fn returns_only_album_files() {
            let (conn, _f) = setup();
            let q = SearchQuery::default();
            let results = execute_manual_album_search(&conn, &"a1".to_string(), &q).unwrap();
            assert_eq!(results.len(), 2);
            let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
            assert!(ids.contains(&"f1"));
            assert!(ids.contains(&"f2"));
        }

        #[test]
        fn sort_by_date_desc() {
            let (conn, _f) = setup();
            let q = SearchQuery { sort_by: SortField::Date, sort_asc: false, ..Default::default() };
            let results = execute_manual_album_search(&conn, &"a1".to_string(), &q).unwrap();
            assert_eq!(results[0].id, "f2");
            assert_eq!(results[1].id, "f1");
        }

        #[test]
        fn ext_filter() {
            let (conn, _f) = setup();
            let q = SearchQuery { extensions: vec!["jpg".into()], ..Default::default() };
            let results = execute_manual_album_search(&conn, &"a1".to_string(), &q).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].id, "f1");
        }
    }
}
