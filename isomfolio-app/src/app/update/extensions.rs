use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::futures;
use iced::Task;
use isomfolio_core::clustering;
use isomfolio_core::extension::ExtensionProcess;
use isomfolio_core::indexing::thumbnail::thumbnail_cache_path;
use isomfolio_core::models::ThumbnailState;

use crate::inference::{EmbedFile, InferenceClient, ManagedInferenceProcess};
use super::LockUnwrap;
use super::super::{App, Msg, ViewMode};

impl App {
    pub(super) fn handle_extension_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::ExtensionsDiscovered(extensions, engine) => {
                let count = extensions.len();
                self.extensions = extensions;
                self.inference_manifest = engine;
                if count > 0 {
                    self.status = format!(
                        "{count} extension{} loaded",
                        if count == 1 { "" } else { "s" }
                    );
                }
                Task::none()
            }

            Msg::RunExtension { addon_idx, method, file_ids } => {
                let Some(ext) = self.extensions.get(addon_idx).cloned() else {
                    return Task::none();
                };
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                let catalog_dir = self.catalog_dir.clone();
                let total = file_ids.len();
                let extension_name = ext.manifest.name.clone();
                self.status = format!("{extension_name}… (0/{total})");

                let requests: Vec<(&str, serde_json::Value)> = file_ids
                    .iter()
                    .map(|id| {
                        let thumb = match self.thumbnails.get(id) {
                            Some(ThumbnailState::Ready(path)) => path.clone(),
                            _ => thumbnail_cache_path(&catalog_dir, id),
                        };
                        (method.as_str(), classify_request_params(id, thumb))
                    })
                    .collect();

                let handle = match ext.send_many(&requests) {
                    Ok(h) => h,
                    Err(e) => {
                        self.status = format!("extension error: {e}");
                        return Task::none();
                    }
                };

                let stream = futures::stream::unfold(
                    ClassifyState { handle, conn, name: extension_name, addon_idx, done: 0, applied: 0, failed: 0 },
                    |mut s| async move {
                        let rx = s.handle.rx.clone();
                        let result =
                            tokio::task::spawn_blocking(move || rx.lock_unwrap().recv()).await;
                        match result {
                            Ok(Ok(Ok(value))) => {
                                if let Some((fid, tags)) = extract_scored_tags(value) {
                                    if !tags.is_empty() && !fid.is_empty() {
                                        let g = s.conn.lock_unwrap();
                                        if let Err(e) = g.insert_pending_tags(&fid, &tags) {
                                            eprintln!("[db] insert_pending_tags failed: {e}");
                                        }
                                        s.applied += 1;
                                    }
                                }
                                s.done += 1;
                                s.into_next_msg()
                            }
                            Ok(Ok(Err(e))) => {
                                eprintln!("[extension] classify error: {e}");
                                s.done += 1;
                                s.failed += 1;
                                s.into_next_msg()
                            }
                            _ => {
                                s.failed += s.handle.total.saturating_sub(s.done);
                                s.done = s.handle.total;
                                s.into_next_msg()
                            }
                        }
                    },
                );
                Task::stream(stream)
            }

            Msg::ExtensionProgress { .. } => Task::none(),

            Msg::ExtensionBatchProgress { name, done, total } => {
                self.status = if total == 100 {
                    format!("{name}… ({done}%)")
                } else {
                    format!("{name}… ({done}/{total})")
                };
                Task::none()
            }

            Msg::FaceClusterProgress { files_done, total, percent } => {
                self.faces.progress = Some(percent as f32 / 100.0);
                self.faces.status = Some(format!("{files_done} / {total} photos"));
                Task::none()
            }

            Msg::ExtensionBatchDone { addon_idx, method, applied, failed } => {
                if failed == 0 {
                    self.status = format!(
                        "{method} done — {applied} file{} updated",
                        if applied == 1 { "" } else { "s" }
                    );
                    return self.refresh_pending_total_task();
                }
                let report_path = self
                    .extensions
                    .get(addon_idx)
                    .and_then(|ext| write_crash_report(ext, applied, failed));
                self.status = match &report_path {
                    Some(path) => format!(
                        "{method} done — {applied} updated, {failed} failed — report: {path}"
                    ),
                    None => format!(
                        "{method} done — {applied} updated, {failed} failed (extension crashed)"
                    ),
                };
                let manifest = self.extensions.get(addon_idx).map(|a| a.manifest.clone());
                if let Some(manifest) = manifest {
                    Task::perform(
                        async move { ExtensionProcess::launch(manifest, None).map(Arc::new).ok() },
                        move |p| Msg::ExtensionRestarted { idx: addon_idx, process: p },
                    )
                } else {
                    Task::none()
                }
            }

            Msg::RunFaceClustering { force_full } => {
                // Ensure a running engine first. If none yet, acquire one
                // (custom URL, or managed local) and come back via InferenceEngineReady.
                if self.inference.is_none() {
                    let custom_url = self
                        .app_settings
                        .inference_custom_url
                        .clone()
                        .filter(|u| !u.trim().is_empty());
                    // Auto mode needs the installed engine binary; custom URL does not.
                    let binary = if custom_url.is_none() {
                        match self.inference_manifest.as_ref() {
                            Some(m) => Some(m.executable.clone()),
                            None => {
                                self.faces.is_clustering = false;
                                self.faces.status =
                                    Some("No inference engine installed".to_string());
                                return Task::none();
                            }
                        }
                    } else {
                        None
                    };
                    self.faces.is_clustering = true;
                    self.faces.progress = None;
                    self.faces.status = Some("Starting inference engine…".to_string());
                    self.task_panel_open = true;
                    let port = self.app_settings.inference_port;
                    let data_dir = isomfolio_core::app_paths::models_dir();
                    // Photo roots — mounted into the engine container (Intel Mac)
                    // so it resolves image paths identically; ignored natively.
                    let mounts: Vec<PathBuf> =
                        self.watchers.iter().map(|(p, _)| PathBuf::from(p)).collect();
                    return Task::perform(
                        acquire_inference_client(custom_url, port, binary, data_dir, mounts),
                        move |client| Msg::InferenceEngineReady { client, force_full },
                    );
                }

                let Some(conn) = self.catalog.clone() else { return Task::none() };
                let client = self.inference.clone().unwrap();
                self.faces.is_clustering = true;
                self.faces.progress = None;
                self.faces.status = Some("Finding people…".to_string());
                self.task_panel_open = true;

                // Sweep stale embeddings, then embed only cache-miss files.
                let (uncached, total_indexed) = {
                    let g = conn.lock_unwrap();
                    if let Err(e) = g.sweep_face_embeddings() {
                        eprintln!("[faces] sweep failed: {e}");
                    }
                    let uncached = g.get_uncached_face_file_paths().unwrap_or_default();
                    let total = g.get_all_file_paths_with_mtimes()
                        .map(|v| v.len())
                        .unwrap_or(uncached.len());
                    (uncached, total)
                };

                if total_indexed == 0 {
                    self.faces.is_clustering = false;
                    self.faces.status = Some("No files to cluster".to_string());
                    return Task::none();
                }

                // Embed uncached files in 50-file batches; clustering then runs
                // over all cached embeddings. Empty chunks → recluster only.
                const BATCH_SIZE: usize = 50;
                let chunks: Vec<Vec<(String, String, i64)>> =
                    uncached.chunks(BATCH_SIZE).map(<[_]>::to_vec).collect();
                let eps = self.app_settings.face_eps;
                let min_pts = self.app_settings.face_min_pts as usize;

                Task::stream(face_cluster_stream(
                    client, conn, chunks, total_indexed, force_full, eps, min_pts,
                ))
            }

            Msg::InferenceEngineReady { client, force_full } => match client {
                Ok(c) => {
                    self.inference = Some(c);
                    Task::done(Msg::RunFaceClustering { force_full })
                }
                Err(e) => {
                    self.faces.is_clustering = false;
                    self.faces.status = Some(format!("Inference engine failed: {e}"));
                    Task::none()
                }
            },

            Msg::FaceClustersBatchDone(summaries) => {
                let count = summaries.len();
                self.faces.clusters = summaries;
                self.faces.status = Some(format!("{count} people found so far…"));
                self.load_face_crops_task()
            }

            Msg::FaceClusteringDone(summaries) => {
                let count = summaries.len();
                self.faces.clusters = summaries;
                self.faces.is_clustering = false;
                self.faces.progress = None;
                self.faces.status = Some(format!("{count} people found"));
                self.load_face_crops_task()
            }

            Msg::FaceClustersLoaded(summaries) => {
                self.faces.clusters = summaries;
                self.load_face_crops_task()
            }

            Msg::FaceCropsReady(handles) => {
                for (cluster_id, handle) in handles {
                    self.faces.crop_handles.insert(cluster_id, handle);
                }
                Task::none()
            }

            Msg::OpenPeopleView => {
                self.view_mode = ViewMode::People;
                self.loupe = super::super::LoupeState::default();
                Task::none()
            }

            Msg::RenameFaceCluster(cluster_id) => {
                let current_name = self
                    .faces
                    .clusters
                    .iter()
                    .find(|c| c.cluster_id == cluster_id)
                    .and_then(|c| c.name.clone())
                    .unwrap_or_default();
                self.faces.rename_cluster_id = Some(cluster_id);
                self.faces.rename_input = current_name;
                Task::none()
            }

            Msg::RenameFaceClusterInputChanged(s) => {
                self.faces.rename_input = s;
                Task::none()
            }

            Msg::ConfirmRenameFaceCluster => {
                let Some(cluster_id) = self.faces.rename_cluster_id.take() else {
                    return Task::none();
                };
                let name = self.faces.rename_input.trim().to_string();
                self.faces.rename_input = String::new();
                if name.is_empty() {
                    return Task::none();
                }
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                if let Some(c) =
                    self.faces.clusters.iter_mut().find(|c| c.cluster_id == cluster_id)
                {
                    c.name = Some(name.clone());
                }
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.rename_face_cluster(&cluster_id, &name).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::NoOp, Msg::DbError),
                )
            }

            Msg::MergeFaceClusters(target_id, source_id) => {
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.faces.clusters.retain(|c| c.cluster_id != source_id);
                if let Some(target) =
                    self.faces.clusters.iter_mut().find(|c| c.cluster_id == target_id)
                {
                    target.file_count += 1;
                }
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        if let Err(e) = g.merge_face_clusters(&target_id, &source_id) {
                            eprintln!("[db] merge_face_clusters failed: {e}");
                        }
                        g.get_face_cluster_summaries().unwrap_or_default()
                    },
                    Msg::FaceClustersLoaded,
                )
            }

            Msg::RemoveFileFromFaceCluster(cluster_id, file_id) => {
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        if let Err(e) = g.remove_file_from_face_cluster(&cluster_id, &file_id) {
                            eprintln!("[db] remove_file_from_face_cluster failed: {e}");
                        }
                        g.get_face_cluster_summaries().unwrap_or_default()
                    },
                    Msg::FaceClustersLoaded,
                )
            }

            Msg::ExtensionRestarted { idx, process } => {
                if self.faces.is_clustering {
                    self.faces.is_clustering = false;
                    self.faces.status =
                        Some("Clustering interrupted — extension restarted".to_string());
                }
                let msg = if let Some(p) = process {
                    if idx < self.extensions.len() {
                        self.extensions[idx] = p;
                    } else {
                        self.extensions.push(p);
                    }
                    "Extension restarted".to_string()
                } else {
                    "Extension restart failed — check logs".to_string()
                };
                if matches!(self.view_mode, super::super::ViewMode::Settings) {
                    self.settings.status = Some(msg);
                } else {
                    self.status = msg;
                }
                Task::none()
            }

            _ => Task::none(),
        }
    }

    pub(super) fn auto_tag_task(&self, new_file_ids: Vec<String>) -> Task<Msg> {
        let preferred =
            self.app_settings.preferred_extension.get("classify").map(|s| s.as_str());
        let classify_idx = self
            .extensions
            .iter()
            .position(|a| {
                a.manifest.capabilities.iter().any(|c| c == "classify")
                    && preferred.map_or(true, |p| a.manifest.name == p)
            })
            .or_else(|| {
                self.extensions.iter().position(|a| {
                    a.manifest.capabilities.iter().any(|c| c == "classify")
                })
            });
        let Some(addon_idx) = classify_idx else { return Task::none() };
        Task::done(Msg::RunExtension {
            addon_idx,
            method: "classify".to_string(),
            file_ids: new_file_ids,
        })
    }

    pub(super) fn load_face_crops_task(&self) -> Task<Msg> {
        let Some(conn) = self.catalog.clone() else { return Task::none() };
        let catalog_dir = self.catalog_dir.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let g = conn.lock_unwrap();
                    let reps = g.get_face_cluster_representatives().unwrap_or_default();
                    let crops = generate_face_crops(&catalog_dir, &reps);
                    crops
                        .into_iter()
                        .filter_map(|(cluster_id, path)| {
                            let bytes = std::fs::read(&path).ok()?;
                            let img = image::load_from_memory(&bytes).ok()?;
                            let rgba = img.into_rgba8();
                            let (w, h) = (rgba.width(), rgba.height());
                            let handle =
                                iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw());
                            Some((cluster_id, handle))
                        })
                        .collect::<Vec<_>>()
                })
                .await
                .unwrap_or_default()
            },
            Msg::FaceCropsReady,
        )
    }
}

fn write_crash_report(
    ext: &ExtensionProcess,
    applied: usize,
    failed: usize,
) -> Option<String> {
    use isomfolio_core::app_paths::crash_reports_dir;
    let dir = crash_reports_dir();
    let _ = std::fs::create_dir_all(&dir);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("{}-{ts}.txt", ext.manifest.name));

    let stderr_lines = ext.last_stderr();
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0);

    let ext_dir =
        ext.manifest.executable.parent().unwrap_or(std::path::Path::new("."));
    let config = isomfolio_core::extension::load_extension_config(ext_dir);
    let config_redacted: serde_json::Map<String, serde_json::Value> = config
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| {
            let is_secret = ext.manifest.config_schema.iter().any(|f| {
                f.key == k
                    && matches!(f.kind, isomfolio_core::extension::ConfigFieldKind::Secret)
            });
            if is_secret { (k, serde_json::Value::String("***".into())) } else { (k, v) }
        })
        .collect();

    let mut report = String::new();
    report.push_str(&format!("Extension: {}\n", ext.manifest.name));
    report.push_str(&format!(
        "OS: {} {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    report.push_str(&format!("CPU cores: {cores}\n"));
    report.push_str(&format!(
        "Config: {}\n",
        serde_json::to_string(&config_redacted).unwrap_or_default()
    ));
    report.push_str(&format!("Applied: {applied}, Failed: {failed}\n"));
    report.push_str("\n--- stderr (last 100 lines) ---\n");
    for line in &stderr_lines {
        report.push_str(line);
        report.push('\n');
    }
    if stderr_lines.is_empty() {
        report.push_str("(no output)\n");
    }

    match std::fs::write(&path, &report) {
        Ok(_) => Some(path.to_string_lossy().into_owned()),
        Err(_) => None,
    }
}

fn generate_face_crops(
    catalog_dir: &str,
    reps: &[(String, String, f64, f64, f64, f64)],
) -> Vec<(String, String)> {
    use isomfolio_core::app_paths::face_crop_path;
    let crop_dir = isomfolio_core::app_paths::face_crop_dir(catalog_dir);
    let _ = std::fs::create_dir_all(&crop_dir);

    reps.iter()
        .filter_map(|(cluster_id, file_path, bx, by, bw, bh)| {
            let out_path = face_crop_path(catalog_dir, cluster_id);
            if std::path::Path::new(&out_path).exists() {
                return Some((cluster_id.clone(), out_path));
            }
            let img = image::open(file_path).ok()?;
            let (iw, ih) = (img.width() as f64, img.height() as f64);
            let x = (bx * iw).max(0.0) as u32;
            let y = (by * ih).max(0.0) as u32;
            let w = (bw * iw).min(iw - x as f64) as u32;
            let h = (bh * ih).min(ih - y as f64) as u32;
            if w == 0 || h == 0 {
                return None;
            }
            let cropped = img.crop_imm(x, y, w, h);
            let thumb = cropped.resize_exact(96, 96, image::imageops::FilterType::Triangle);
            thumb.save(&out_path).ok()?;
            Some((cluster_id.clone(), out_path))
        })
        .collect()
}

struct ClassifyState {
    handle: isomfolio_core::extension::BatchHandle,
    conn: Arc<std::sync::Mutex<isomfolio_core::Catalog>>,
    name: String,
    addon_idx: usize,
    done: usize,
    applied: usize,
    failed: usize,
}

impl ClassifyState {
    fn into_next_msg(self) -> Option<(Msg, Self)> {
        if self.done >= self.handle.total {
            Some((
                Msg::ExtensionBatchDone {
                    addon_idx: self.addon_idx,
                    method: "classify".into(),
                    applied: self.applied,
                    failed: self.failed,
                },
                self,
            ))
        } else {
            Some((
                Msg::ExtensionBatchProgress {
                    name: self.name.clone(),
                    done: self.done,
                    total: self.handle.total,
                },
                self,
            ))
        }
    }
}

#[derive(serde::Serialize)]
struct ClassifyRequest<'a> {
    file_id: &'a str,
    thumbnail_path: String,
}

#[derive(serde::Deserialize)]
struct ClassifyResponse {
    file_id: String,
    #[serde(default)]
    tags: Vec<ClassifyTag>,
}

#[derive(serde::Deserialize)]
struct ClassifyTag {
    tag: String,
    confidence: Option<f32>,
}

fn extract_scored_tags(result: serde_json::Value) -> Option<(String, Vec<(String, Option<f32>)>)> {
    let resp = serde_json::from_value::<ClassifyResponse>(result).ok()?;
    Some((resp.file_id, resp.tags.into_iter().map(|t| (t.tag, t.confidence)).collect()))
}

fn classify_request_params(file_id: &str, thumbnail_path: String) -> serde_json::Value {
    serde_json::to_value(ClassifyRequest { file_id, thumbnail_path }).unwrap_or_default()
}

/// Spawn the managed local engine, wait for it to become healthy (generous —
/// the first run downloads models), and wrap it in a client.
/// Obtain a healthy inference client: a user-supplied remote URL if set,
/// otherwise the managed local engine (native, or Docker on Intel Mac).
async fn acquire_inference_client(
    custom_url: Option<String>,
    port: u16,
    binary: Option<PathBuf>,
    data_dir: PathBuf,
    mounts: Vec<PathBuf>,
) -> Result<Arc<InferenceClient>, String> {
    let client = if let Some(url) = custom_url {
        let _ = (&binary, &data_dir, &mounts);
        InferenceClient::remote(url.trim()).map_err(|e| e.to_string())?
    } else {
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        let proc = {
            let _ = &binary; // unused on the Docker path
            ManagedInferenceProcess::spawn_docker(port, &data_dir, &mounts)
                .map_err(|e| e.to_string())?
        };
        #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
        let proc = {
            let _ = &mounts; // mounts only matter for the Docker path
            let binary = binary.ok_or("no engine binary")?;
            ManagedInferenceProcess::spawn(&binary, port, &data_dir).map_err(|e| e.to_string())?
        };
        InferenceClient::managed(proc).map_err(|e| e.to_string())?
    };
    client
        .wait_healthy(Duration::from_secs(600))
        .await
        .map_err(|e| e.to_string())?;
    Ok(Arc::new(client))
}

/// Drive the engine: embed each batch of uncached files, persist embeddings,
/// then cluster all cached embeddings once at the end.
#[allow(clippy::too_many_arguments)]
fn face_cluster_stream(
    client: Arc<InferenceClient>,
    conn: Arc<std::sync::Mutex<isomfolio_core::Catalog>>,
    chunks: Vec<Vec<(String, String, i64)>>,
    total_indexed: usize,
    force_full: bool,
    eps: f32,
    min_pts: usize,
) -> impl futures::Stream<Item = Msg> {
    enum Stage {
        Embed(usize),
        Cluster,
        Done,
    }

    struct St {
        client: Arc<InferenceClient>,
        conn: Arc<std::sync::Mutex<isomfolio_core::Catalog>>,
        chunks: Vec<Vec<(String, String, i64)>>,
        total_indexed: usize,
        force_full: bool,
        eps: f32,
        min_pts: usize,
        files_sent: usize,
        stage: Stage,
    }

    let total_batches = chunks.len();
    let stage = if chunks.is_empty() { Stage::Cluster } else { Stage::Embed(0) };
    let state =
        St { client, conn, chunks, total_indexed, force_full, eps, min_pts, files_sent: 0, stage };

    futures::stream::unfold(state, move |mut s| async move {
        match s.stage {
            Stage::Embed(i) => {
                let chunk = s.chunks[i].clone();
                let files: Vec<EmbedFile> = chunk
                    .iter()
                    .map(|(id, path, mtime)| EmbedFile {
                        file_id: id.clone(),
                        path: path.clone(),
                        mtime: *mtime,
                    })
                    .collect();
                let batch_len = files.len();

                match s.client.embed(files).await {
                    Ok(resp) => {
                        let mtimes: std::collections::HashMap<String, i64> =
                            chunk.iter().map(|(id, _, m)| (id.clone(), *m)).collect();
                        let conn = s.conn.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            let g = conn.lock_unwrap();
                            for r in resp.results {
                                let mtime = mtimes.get(&r.file_id).copied().unwrap_or(0);
                                let faces: Vec<(f64, f64, f64, f64, Vec<f32>)> = r
                                    .faces
                                    .into_iter()
                                    .map(|f| (f.bbox.x, f.bbox.y, f.bbox.w, f.bbox.h, f.vec))
                                    .collect();
                                if let Err(e) = g.insert_face_embeddings(&r.file_id, mtime, &faces) {
                                    eprintln!("[faces] insert_face_embeddings failed: {e}");
                                }
                            }
                        })
                        .await;

                        s.files_sent += batch_len;
                        let next = i + 1;
                        s.stage = if next < s.chunks.len() { Stage::Embed(next) } else { Stage::Cluster };
                        // Embedding occupies the 0–80% band; clustering is the tail.
                        let percent = (next * 80 / total_batches) as u8;
                        let files_done = s.files_sent;
                        let total = s.total_indexed;
                        Some((Msg::FaceClusterProgress { files_done, total, percent }, s))
                    }
                    Err(e) => {
                        eprintln!("[faces] embed batch {} failed: {e}", i + 1);
                        // Cluster whatever embeddings we already have.
                        s.stage = Stage::Cluster;
                        Some((Msg::NoOp, s))
                    }
                }
            }
            Stage::Cluster => {
                let conn = s.conn.clone();
                let force_full = s.force_full;
                let eps = s.eps;
                let min_pts = s.min_pts;
                let summaries = tokio::task::spawn_blocking(move || {
                    let g = conn.lock_unwrap();
                    let rows = g.load_all_face_embeddings().unwrap_or_default();
                    if rows.is_empty() {
                        return g.get_face_cluster_summaries().unwrap_or_default();
                    }
                    let embeddings: Vec<Vec<f32>> = rows.iter().map(|r| r.vec.clone()).collect();
                    let centroids = g.load_face_centroids().unwrap_or_default();

                    // Incremental: assign to known people. Full: rediscover via DBSCAN.
                    let labels = if !force_full && !centroids.is_empty() {
                        let cvecs: Vec<Vec<f32>> =
                            centroids.iter().map(|(_, v)| v.clone()).collect();
                        clustering::assign_to_centroids(&embeddings, &cvecs, eps)
                    } else {
                        clustering::dbscan(&embeddings, eps, min_pts)
                    };

                    let assembly = clustering::assemble_clusters(&rows, &labels);
                    if let Err(e) = g.save_face_clusters(&assembly.members) {
                        eprintln!("[db] save_face_clusters failed: {e}");
                    }
                    // Keep existing centroids on an incremental run.
                    if force_full || centroids.is_empty() {
                        if let Err(e) = g.save_face_centroids(&assembly.centroids) {
                            eprintln!("[db] save_face_centroids failed: {e}");
                        }
                    }
                    g.get_face_cluster_summaries().unwrap_or_default()
                })
                .await
                .unwrap_or_default();

                s.stage = Stage::Done;
                Some((Msg::FaceClusteringDone(summaries), s))
            }
            Stage::Done => None,
        }
    })
}
