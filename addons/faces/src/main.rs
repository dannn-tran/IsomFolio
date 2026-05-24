mod cluster;
mod model;

use std::io::{self, BufRead, Write};

use rusqlite::Connection as SqliteConn;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use model::{FaceModels, MODEL_VERSION};

#[derive(Deserialize)]
struct Request {
    id: u64,
    method: String,
    params: Value,
}

#[derive(Serialize)]
struct Response {
    id: u64,
    result: Value,
}

#[derive(Serialize)]
struct ErrorResponse {
    id: u64,
    error: String,
}

fn emit_log(out: &mut impl Write, level: &str, msg: &str) {
    let _ = writeln!(out, "{}", serde_json::json!({"type":"log","level":level,"message":msg}));
    let _ = out.flush();
}

fn emit_progress(out: &mut impl Write, id: u64, percent: u32) {
    let _ = writeln!(out, "{}", serde_json::json!({"type":"progress","id":id,"percent":percent}));
    let _ = out.flush();
}

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let models_dir = std::env::var("ISOMFOLIO_MODELS_DIR").unwrap_or_else(|_| ".".to_string());

    emit_log(&mut out, "info", "loading face models…");
    let models = match FaceModels::load(&models_dir, &mut out) {
        Ok(m) => m,
        Err(e) => {
            emit_log(&mut out, "error", &format!("model init failed: {e}"));
            return;
        }
    };

    let state_db = match open_state_db(&models_dir) {
        Ok(db) => db,
        Err(e) => {
            emit_log(&mut out, "error", &format!("state DB init failed: {e}"));
            return;
        }
    };

    emit_log(&mut out, "info", "ready");

    let _ = writeln!(
        out,
        "{}",
        serde_json::json!({
            "type": "hello",
            "protocol_version": 1,
            "addon_api_version": 1,
            "capabilities": ["cluster_faces"],
        })
    );
    let _ = out.flush();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<Request>(line) {
            Ok(req) => {
                let resp = match req.method.as_str() {
                    "cluster_faces" => {
                        match handle_cluster_faces(&models, &state_db, &req.params, req.id, &mut out) {
                            Ok(r) => serde_json::to_string(&Response { id: req.id, result: r }).unwrap(),
                            Err(e) => serde_json::to_string(&ErrorResponse { id: req.id, error: e }).unwrap(),
                        }
                    }
                    m => serde_json::to_string(&ErrorResponse {
                        id: req.id,
                        error: format!("unknown method: {m}"),
                    })
                    .unwrap(),
                };
                let _ = writeln!(out, "{resp}");
                let _ = out.flush();
            }
            Err(e) => eprintln!("[faces] parse error: {e}"),
        }
    }
}

fn open_state_db(models_dir: &str) -> Result<SqliteConn, String> {
    let dir = std::path::PathBuf::from(models_dir).join("faces");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let db = SqliteConn::open(dir.join("state.db")).map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
        .map_err(|e| e.to_string())?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS face_embeddings (
            id           INTEGER PRIMARY KEY,
            file_id      TEXT NOT NULL,
            file_mtime   INTEGER NOT NULL,
            model_version TEXT NOT NULL,
            bbox_x       REAL NOT NULL,
            bbox_y       REAL NOT NULL,
            bbox_w       REAL NOT NULL,
            bbox_h       REAL NOT NULL,
            vec          BLOB NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_fe_key
            ON face_embeddings (file_id, file_mtime, model_version);",
    )
    .map_err(|e| e.to_string())?;
    Ok(db)
}

fn handle_cluster_faces(
    models: &FaceModels,
    db: &SqliteConn,
    params: &Value,
    req_id: u64,
    out: &mut impl Write,
) -> Result<Value, String> {
    let files = params
        .get("files")
        .and_then(|v| v.as_array())
        .ok_or("missing files array")?;

    if files.is_empty() {
        return Ok(serde_json::json!({"clusters": []}));
    }

    let total = files.len();
    emit_log(out, "info", &format!("processing {total} files…"));

    for (i, file) in files.iter().enumerate() {
        let file_id = file.get("file_id").and_then(|v| v.as_str()).unwrap_or("");
        let image_path = file.get("image_path").and_then(|v| v.as_str()).unwrap_or("");
        let file_mtime = file.get("file_mtime").and_then(|v| v.as_i64()).unwrap_or(0);

        let percent = ((i as f64 / total as f64) * 80.0) as u32;
        emit_progress(out, req_id, percent);

        if is_cached(db, file_id, file_mtime) {
            continue;
        }

        if image_path.is_empty() || !std::path::Path::new(image_path).exists() {
            continue;
        }

        let img = match image::open(image_path) {
            Ok(img) => img.to_rgb8(),
            Err(_) => continue,
        };

        match models.detect(&img) {
            Ok(faces) => {
                // Delete stale embeddings for this file (mtime changed)
                let _ = db.execute(
                    "DELETE FROM face_embeddings WHERE file_id = ? AND (file_mtime != ? OR model_version != ?)",
                    rusqlite::params![file_id, file_mtime, MODEL_VERSION],
                );

                for face in &faces {
                    match models.embed(&img, face) {
                        Ok(vec) => {
                            let blob = floats_to_bytes(&vec);
                            let _ = db.execute(
                                "INSERT INTO face_embeddings
                                 (file_id, file_mtime, model_version, bbox_x, bbox_y, bbox_w, bbox_h, vec)
                                 VALUES (?,?,?,?,?,?,?,?)",
                                rusqlite::params![
                                    file_id,
                                    file_mtime,
                                    MODEL_VERSION,
                                    face.bbox_x,
                                    face.bbox_y,
                                    face.bbox_w,
                                    face.bbox_h,
                                    blob,
                                ],
                            );
                        }
                        Err(e) => emit_log(out, "warn", &format!("embed failed for {file_id}: {e}")),
                    }
                }
            }
            Err(e) => emit_log(out, "warn", &format!("detect failed for {file_id}: {e}")),
        }
    }

    emit_progress(out, req_id, 82);
    emit_log(out, "info", "clustering faces…");

    // Load all embeddings from state DB
    struct Row {
        file_id: String,
        bbox_x: f32,
        bbox_y: f32,
        bbox_w: f32,
        bbox_h: f32,
        vec: Vec<f32>,
    }

    let mut stmt = db
        .prepare(
            "SELECT file_id, bbox_x, bbox_y, bbox_w, bbox_h, vec
             FROM face_embeddings
             WHERE model_version = ?",
        )
        .map_err(|e| e.to_string())?;

    let rows: Vec<Row> = stmt
        .query_map(rusqlite::params![MODEL_VERSION], |row| {
            let blob: Vec<u8> = row.get(5)?;
            Ok(Row {
                file_id: row.get(0)?,
                bbox_x: row.get(1)?,
                bbox_y: row.get(2)?,
                bbox_w: row.get(3)?,
                bbox_h: row.get(4)?,
                vec: bytes_to_floats(&blob),
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    emit_progress(out, req_id, 90);

    if rows.is_empty() {
        emit_progress(out, req_id, 100);
        return Ok(serde_json::json!({"clusters": []}));
    }

    let embeddings: Vec<Vec<f32>> = rows.iter().map(|r| r.vec.clone()).collect();
    let labels = cluster::dbscan(&embeddings, 0.4, 2);

    emit_progress(out, req_id, 95);

    // Group by cluster label
    let max_label = labels.iter().copied().max().unwrap_or(-1);
    let mut cluster_members: Vec<Vec<Value>> =
        (0..=(max_label.max(0) as usize)).map(|_| Vec::new()).collect();

    for (i, &label) in labels.iter().enumerate() {
        if label < 0 {
            continue;
        }
        let r = &rows[i];
        cluster_members[label as usize].push(serde_json::json!({
            "file_id": r.file_id,
            "bbox": {"x": r.bbox_x, "y": r.bbox_y, "w": r.bbox_w, "h": r.bbox_h},
        }));
    }

    // Sort by cluster size desc, then re-number
    let mut clusters: Vec<Vec<Value>> =
        cluster_members.into_iter().filter(|m| !m.is_empty()).collect();
    clusters.sort_by(|a, b| b.len().cmp(&a.len()));

    let result: Vec<Value> = clusters
        .into_iter()
        .enumerate()
        .map(|(i, members)| {
            serde_json::json!({
                "id": format!("person-{i}"),
                "members": members,
            })
        })
        .collect();

    emit_progress(out, req_id, 100);
    emit_log(out, "info", &format!("found {} people", result.len()));

    Ok(serde_json::json!({"clusters": result}))
}

fn is_cached(db: &SqliteConn, file_id: &str, file_mtime: i64) -> bool {
    db.query_row(
        "SELECT 1 FROM face_embeddings WHERE file_id = ? AND file_mtime = ? AND model_version = ? LIMIT 1",
        rusqlite::params![file_id, file_mtime, MODEL_VERSION],
        |_| Ok(()),
    )
    .is_ok()
}

fn floats_to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn bytes_to_floats(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}
