mod cluster;
mod model;

use std::io::{self, BufRead, Write};

use isomfolio_addon_sdk as sdk;
use rusqlite::Connection as SqliteConn;
use serde::Deserialize;
use serde_json::Value;

use model::{FaceModels, MODEL_VERSION};

#[derive(Deserialize, Default)]
struct Config {
    #[serde(default = "default_eps")]
    eps: f32,
    #[serde(default = "default_min_pts")]
    min_pts: usize,
}

fn default_eps() -> f32 { 0.4 }
fn default_min_pts() -> usize { 2 }

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    sdk::send_hello(&mut out, &["cluster_faces"]);

    let config: Config = sdk::load_config(&mut out);
    let models_dir = std::env::var("ISOMFOLIO_MODELS_DIR").unwrap_or_else(|_| ".".to_string());

    sdk::emit_log(&mut out, "info", "loading face models…");
    let models = match FaceModels::load(&models_dir, &mut out) {
        Ok(m) => m,
        Err(e) => {
            sdk::emit_log(&mut out, "error", &format!("model init failed: {e}"));
            return;
        }
    };

    let state_db = match open_state_db(&models_dir) {
        Ok(db) => db,
        Err(e) => {
            sdk::emit_log(&mut out, "error", &format!("state DB init failed: {e}"));
            return;
        }
    };

    sdk::emit_log(&mut out, "info", "ready");

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<sdk::Request>(line) {
            Ok(req) => match req.method.as_str() {
                "cluster_faces" => {
                    match handle_cluster_faces(&models, &state_db, &config, &req.params, req.id, &mut out) {
                        Ok(r) => sdk::send_response(&mut out, req.id, r),
                        Err(e) => sdk::send_error(&mut out, req.id, e),
                    }
                }
                m => sdk::send_error(&mut out, req.id, format!("unknown method: {m}")),
            },
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
            ON face_embeddings (file_id, file_mtime, model_version);
        CREATE TABLE IF NOT EXISTS cluster_centroids (
            cluster_id   TEXT PRIMARY KEY,
            centroid     BLOB NOT NULL
        );",
    )
    .map_err(|e| e.to_string())?;
    Ok(db)
}

fn handle_cluster_faces(
    models: &FaceModels,
    db: &SqliteConn,
    config: &Config,
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
    sdk::emit_log(out, "info", &format!("processing {total} files…"));

    for (i, file) in files.iter().enumerate() {
        let file_id = file.get("file_id").and_then(|v| v.as_str()).unwrap_or("");
        let image_path = file.get("image_path").and_then(|v| v.as_str()).unwrap_or("");
        let file_mtime = file.get("file_mtime").and_then(|v| v.as_i64()).unwrap_or(0);

        let percent = ((i as f64 / total as f64) * 80.0) as u32;
        sdk::emit_progress(out, req_id, percent);

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
                        Err(e) => sdk::emit_log(out, "warn", &format!("embed failed for {file_id}: {e}")),
                    }
                }
            }
            Err(e) => sdk::emit_log(out, "warn", &format!("detect failed for {file_id}: {e}")),
        }
    }

    sdk::emit_progress(out, req_id, 82);

    let rows = load_all_embeddings(db)?;
    if rows.is_empty() {
        sdk::emit_progress(out, req_id, 100);
        return Ok(serde_json::json!({"clusters": [], "noise": []}));
    }

    let force_full = params.get("force_full").and_then(|v| v.as_bool()).unwrap_or(false);
    let centroids = load_centroids(db);
    let use_incremental = !force_full && !centroids.is_empty();

    let labels = if use_incremental {
        sdk::emit_log(out, "info", &format!("incremental assignment against {} centroids…", centroids.len()));
        assign_to_centroids(&rows, &centroids, config.eps)
    } else {
        sdk::emit_log(out, "info", "full DBSCAN clustering…");
        let embeddings: Vec<Vec<f32>> = rows.iter().map(|r| r.vec.clone()).collect();
        cluster::dbscan(&embeddings, config.eps, config.min_pts)
    };

    sdk::emit_progress(out, req_id, 95);

    let max_label = labels.iter().copied().max().unwrap_or(-1);
    let mut cluster_members: Vec<Vec<(usize, Value)>> =
        (0..=(max_label.max(0) as usize)).map(|_| Vec::new()).collect();

    let mut noise_members: Vec<Value> = Vec::new();
    for (i, &label) in labels.iter().enumerate() {
        let r = &rows[i];
        let member = serde_json::json!({
            "file_id": r.file_id,
            "bbox": {"x": r.bbox_x, "y": r.bbox_y, "w": r.bbox_w, "h": r.bbox_h},
        });
        if label < 0 {
            noise_members.push(member);
        } else {
            cluster_members[label as usize].push((i, member));
        }
    }

    let mut clusters: Vec<Vec<(usize, Value)>> =
        cluster_members.into_iter().filter(|m| !m.is_empty()).collect();
    clusters.sort_by_key(|c| std::cmp::Reverse(c.len()));

    if !use_incremental {
        save_centroids(db, &rows, &clusters);
    }

    let result: Vec<Value> = clusters
        .into_iter()
        .map(|members| {
            let json_members: Vec<Value> = members.into_iter().map(|(_, v)| v).collect();
            let id = stable_cluster_id(&json_members);
            serde_json::json!({ "id": id, "members": json_members })
        })
        .collect();

    sdk::emit_progress(out, req_id, 100);
    sdk::emit_log(out, "info", &format!("found {} people, {} unclustered faces", result.len(), noise_members.len()));

    Ok(serde_json::json!({"clusters": result, "noise": noise_members}))
}

struct EmbeddingRow {
    file_id: String,
    bbox_x: f32,
    bbox_y: f32,
    bbox_w: f32,
    bbox_h: f32,
    vec: Vec<f32>,
}

fn load_all_embeddings(db: &SqliteConn) -> Result<Vec<EmbeddingRow>, String> {
    let mut stmt = db
        .prepare(
            "SELECT file_id, bbox_x, bbox_y, bbox_w, bbox_h, vec
             FROM face_embeddings WHERE model_version = ?",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![MODEL_VERSION], |row| {
            let blob: Vec<u8> = row.get(5)?;
            Ok(EmbeddingRow {
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
    Ok(rows)
}

fn load_centroids(db: &SqliteConn) -> Vec<(String, Vec<f32>)> {
    let mut stmt = match db.prepare("SELECT cluster_id, centroid FROM cluster_centroids") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    stmt.query_map([], |row| {
        let blob: Vec<u8> = row.get(1)?;
        Ok((row.get::<_, String>(0)?, bytes_to_floats(&blob)))
    })
    .ok()
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

fn save_centroids(db: &SqliteConn, rows: &[EmbeddingRow], clusters: &[Vec<(usize, Value)>]) {
    let _ = db.execute("DELETE FROM cluster_centroids", []);
    for cluster in clusters {
        if cluster.is_empty() { continue; }
        let dim = rows[cluster[0].0].vec.len();
        let mut centroid = vec![0.0f32; dim];
        for (idx, _) in cluster {
            for (j, &v) in rows[*idx].vec.iter().enumerate() {
                centroid[j] += v;
            }
        }
        let n = cluster.len() as f32;
        for v in &mut centroid {
            *v /= n;
        }
        let norm: f32 = centroid.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut centroid {
                *v /= norm;
            }
        }
        let json_members: Vec<Value> = cluster.iter().map(|(_, v)| v.clone()).collect();
        let id = stable_cluster_id(&json_members);
        let blob = floats_to_bytes(&centroid);
        let _ = db.execute(
            "INSERT OR REPLACE INTO cluster_centroids (cluster_id, centroid) VALUES (?, ?)",
            rusqlite::params![id, blob],
        );
    }
}

fn assign_to_centroids(rows: &[EmbeddingRow], centroids: &[(String, Vec<f32>)], eps: f32) -> Vec<i32> {
    let mut labels = vec![-1i32; rows.len()];
    for (i, row) in rows.iter().enumerate() {
        let mut best_sim = 0.0f32;
        let mut best_label = -1i32;
        for (ci, (_, centroid)) in centroids.iter().enumerate() {
            let sim = cosine_sim(&row.vec, centroid);
            if sim > best_sim {
                best_sim = sim;
                best_label = ci as i32;
            }
        }
        if best_sim >= (1.0 - eps) {
            labels[i] = best_label;
        }
    }
    labels
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}

fn stable_cluster_id(members: &[Value]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut keys: Vec<String> = members
        .iter()
        .map(|m| {
            let fid = m.get("file_id").and_then(|v| v.as_str()).unwrap_or("");
            let bbox = m.get("bbox").unwrap_or(&Value::Null);
            let x = bbox.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let y = bbox.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
            format!("{fid}:{x:.1}:{y:.1}")
        })
        .collect();
    keys.sort();

    let mut hasher = DefaultHasher::new();
    for k in &keys {
        k.hash(&mut hasher);
    }
    format!("face-{:016x}", hasher.finish())
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
