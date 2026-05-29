use std::sync::Arc;
use std::time::Duration;

use iced::futures;
use iced::Task;
use isomfolio_core::extension::ExtensionProcess;
use isomfolio_core::indexing::thumbnail::thumbnail_cache_path;
use isomfolio_core::models::{FaceClusterMember, ThumbnailState};

use super::LockUnwrap;
use super::super::{App, Msg, ViewMode};

impl App {
    pub(super) fn handle_extension_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::ExtensionsDiscovered(extensions) => {
                let count = extensions.len();
                self.extensions = extensions;
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
                let msg = if total == 100 {
                    format!("{name}… ({done}%)")
                } else {
                    format!("{name}… ({done}/{total})")
                };
                if name.contains("faces") || name.contains("Clustering") {
                    self.faces.status = Some(msg);
                } else {
                    self.status = msg;
                }
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
                        async move { ExtensionProcess::launch(manifest).map(Arc::new).ok() },
                        move |p| Msg::ExtensionRestarted { idx: addon_idx, process: p },
                    )
                } else {
                    Task::none()
                }
            }

            Msg::RunFaceClustering { force_full } => {
                let Some(ext) = self
                    .extensions
                    .iter()
                    .find(|a| a.manifest.capabilities.contains(&"cluster_faces".to_string()))
                    .cloned()
                else {
                    self.faces.status =
                        Some("No face clustering extension installed".to_string());
                    return Task::none();
                };
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.faces.status = Some("Clustering faces… (0%)".to_string());

                let files = {
                    let g = conn.lock_unwrap();
                    g.get_all_file_paths_with_mtimes().unwrap_or_default()
                };
                let params = cluster_faces_request_params(&files, force_full);

                let handle = match ext.send("cluster_faces", params) {
                    Ok(h) => h,
                    Err(e) => {
                        self.faces.status = Some(format!("face clustering error: {e}"));
                        return Task::none();
                    }
                };

                enum ClusterPoll {
                    Progress(u8),
                    Done(Vec<isomfolio_core::models::FaceClusterSummary>),
                    Failed(String),
                    Pending,
                }

                let stream = futures::stream::unfold(
                    (handle, conn, false),
                    |(handle, conn, done)| async move {
                        if done {
                            return None;
                        }
                        let conn2 = conn.clone();
                        let (poll, handle) = tokio::task::spawn_blocking(
                            move || -> Option<(ClusterPoll, _)> {
                                let result = match handle
                                    .progress_rx
                                    .recv_timeout(Duration::from_millis(200))
                                {
                                    Ok(percent) => {
                                        return Some((ClusterPoll::Progress(percent), handle));
                                    }
                                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                        match handle.result_rx.try_recv() {
                                            Ok(r) => r,
                                            Err(_) => return Some((ClusterPoll::Pending, handle)),
                                        }
                                    }
                                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                        handle.result_rx.recv().ok()?
                                    }
                                };
                                match result {
                                    Ok(value) => {
                                        let clusters = parse_cluster_response(value);
                                        let g = conn2.lock_unwrap();
                                        if let Err(e) = g.save_face_clusters(&clusters) {
                                            eprintln!("[db] save_face_clusters failed: {e}");
                                        }
                                        let summaries =
                                            g.get_face_cluster_summaries().unwrap_or_default();
                                        Some((ClusterPoll::Done(summaries), handle))
                                    }
                                    Err(e) => Some((ClusterPoll::Failed(e), handle)),
                                }
                            },
                        )
                        .await
                        .ok()
                        .flatten()?;

                        match poll {
                            ClusterPoll::Progress(percent) => Some((
                                Msg::ExtensionBatchProgress {
                                    name: "Clustering faces".into(),
                                    done: percent as usize,
                                    total: 100,
                                },
                                (handle, conn, false),
                            )),
                            ClusterPoll::Done(summaries) => {
                                Some((Msg::FaceClusteringDone(summaries), (handle, conn, true)))
                            }
                            ClusterPoll::Failed(e) => {
                                eprintln!("[faces] cluster_faces error: {e}");
                                Some((Msg::FaceClusteringDone(Vec::new()), (handle, conn, true)))
                            }
                            ClusterPoll::Pending => Some((Msg::NoOp, (handle, conn, false))),
                        }
                    },
                );
                Task::stream(stream)
            }

            Msg::FaceClusteringDone(summaries) => {
                let count = summaries.len();
                self.faces.clusters = summaries;
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
                if self.settings.show {
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

#[derive(serde::Serialize)]
struct ClusterFacesRequest {
    files: Vec<ClusterFaceFile>,
    force_full: bool,
}

#[derive(serde::Serialize)]
struct ClusterFaceFile {
    file_id: String,
    image_path: String,
    file_mtime: i64,
}

#[derive(serde::Deserialize, Default)]
struct ClusterFacesResponse {
    #[serde(default)]
    clusters: Vec<ClusterGroup>,
    #[serde(default)]
    noise: Vec<ClusterMemberDto>,
}

#[derive(serde::Deserialize)]
struct ClusterGroup {
    id: String,
    #[serde(default)]
    members: Vec<ClusterMemberDto>,
}

#[derive(serde::Deserialize)]
struct ClusterMemberDto {
    file_id: String,
    #[serde(default)]
    bbox: BboxDto,
}

#[derive(serde::Deserialize, Default)]
struct BboxDto {
    #[serde(default)]
    x: f64,
    #[serde(default)]
    y: f64,
    #[serde(default)]
    w: f64,
    #[serde(default)]
    h: f64,
}

fn extract_scored_tags(result: serde_json::Value) -> Option<(String, Vec<(String, Option<f32>)>)> {
    let resp = serde_json::from_value::<ClassifyResponse>(result).ok()?;
    Some((resp.file_id, resp.tags.into_iter().map(|t| (t.tag, t.confidence)).collect()))
}

fn classify_request_params(file_id: &str, thumbnail_path: String) -> serde_json::Value {
    serde_json::to_value(ClassifyRequest { file_id, thumbnail_path }).unwrap_or_default()
}

fn cluster_faces_request_params(
    files: &[(String, String, i64)],
    force_full: bool,
) -> serde_json::Value {
    let files = files
        .iter()
        .map(|(id, path, mtime)| ClusterFaceFile {
            file_id: id.clone(),
            image_path: path.clone(),
            file_mtime: *mtime,
        })
        .collect();
    serde_json::to_value(ClusterFacesRequest { files, force_full }).unwrap_or_default()
}

const UNKNOWN_FACES_CLUSTER: &str = "face-unknown";

fn parse_cluster_response(v: serde_json::Value) -> Vec<FaceClusterMember> {
    let resp: ClusterFacesResponse = serde_json::from_value(v).unwrap_or_default();
    let mut rows: Vec<FaceClusterMember> = resp
        .clusters
        .into_iter()
        .filter(|c| !c.id.is_empty())
        .flat_map(|c| {
            let cluster_id = c.id;
            c.members.into_iter().map(move |m| FaceClusterMember {
                cluster_id: cluster_id.clone(),
                file_id: m.file_id,
                bbox_x: m.bbox.x,
                bbox_y: m.bbox.y,
                bbox_w: m.bbox.w,
                bbox_h: m.bbox.h,
            })
        })
        .collect();
    for m in resp.noise {
        rows.push(FaceClusterMember {
            cluster_id: UNKNOWN_FACES_CLUSTER.to_string(),
            file_id: m.file_id,
            bbox_x: m.bbox.x,
            bbox_y: m.bbox.y,
            bbox_w: m.bbox.w,
            bbox_h: m.bbox.h,
        });
    }
    rows
}
