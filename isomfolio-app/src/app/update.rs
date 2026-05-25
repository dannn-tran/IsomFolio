use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::Task;
use iced::futures;

use isomfolio_core::addon::{
    discover_addons, install_addon_package, load_addon_config, save_addon_config,
    uninstall_addon, AddonProcess,
};
use isomfolio_core::app_paths::addons_dir;
use isomfolio_core::indexing::thumbnail::thumbnail_cache_path;
use isomfolio_core::models::ThumbnailState;
use isomfolio_core::file_index::compute_file_id;
use isomfolio_core::indexing::types::FileEvent;
use isomfolio_core::models::{Album, AlbumKind, FaceClusterMember, SortField};
use isomfolio_core::path_utils::{is_catalog_dir, is_under_catalog_dir, normalize_path};

use super::{
    unix_to_date_str, App, ContextMenuState, ContextMenuTarget, CriteriaState, DetailState,
    DragState, Msg, SettingsState, SidebarItem, TagBrowserState, ViewMode, ALBUM_ITEM_HEIGHT,
    SIDEBAR_ALBUMS_BASE_Y, SIDEBAR_HANDLE_WIDTH,
};
use isomfolio_core::app_paths::db_path;

impl App {
    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::CatalogReady => {
                self.start_thumbnail_pool();
                let sidebar_task = self.load_sidebar_task();
                let addon_task = Task::perform(
                    async move {
                        let dir = addons_dir();
                        let manifests = discover_addons(&dir);
                        manifests
                            .into_iter()
                            .filter_map(|m| {
                                AddonProcess::launch(m)
                                    .map(Arc::new)
                                    .map_err(|e| eprintln!("[addon] launch failed: {e}"))
                                    .ok()
                            })
                            .collect::<Vec<_>>()
                    },
                    Msg::AddonsDiscovered,
                );
                let face_task = if let Some(conn) = self.catalog.clone() {
                    Task::perform(
                        async move {
                            let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                            g.get_face_cluster_summaries().unwrap_or_default()
                        },
                        Msg::FaceClustersLoaded,
                    )
                } else {
                    Task::none()
                };
                Task::batch([sidebar_task, addon_task, face_task])
            }

            Msg::AddonsDiscovered(addons) => {
                let count = addons.len();
                self.addons = addons;
                if count > 0 {
                    self.status = format!("{count} addon{} loaded", if count == 1 { "" } else { "s" });
                }
                Task::none()
            }

            Msg::RunAddon { addon_idx, method, file_ids } => {
                let Some(addon) = self.addons.get(addon_idx).cloned() else {
                    return Task::none();
                };
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                let catalog_dir = self.catalog_dir.clone();
                let total = file_ids.len();
                let addon_name = addon.manifest.name.clone();
                self.status = format!("{addon_name}… (0/{total})");

                let requests: Vec<(&str, serde_json::Value)> = file_ids
                    .iter()
                    .map(|id| {
                        let thumb = match self.thumbnails.get(id) {
                            Some(ThumbnailState::Ready(path)) => path.clone(),
                            _ => thumbnail_cache_path(&catalog_dir, id),
                        };
                        (method.as_str(), serde_json::json!({ "file_id": id, "thumbnail_path": thumb }))
                    })
                    .collect();

                let handle = match addon.send_many(&requests) {
                    Ok(h) => h,
                    Err(e) => {
                        self.status = format!("addon error: {e}");
                        return Task::none();
                    }
                };

                let stream = futures::stream::unfold(
                    (handle, conn, addon_name, addon_idx, 0usize, 0usize, 0usize),
                    |(handle, conn, name, addon_idx, mut done, mut applied, mut failed)| async move {
                        let rx = handle.rx.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            rx.lock().unwrap_or_else(|e| e.into_inner()).recv()
                        }).await;
                        match result {
                            Ok(Ok(Ok(value))) => {
                                let tags = extract_scored_tags(&value);
                                if !tags.is_empty() {
                                    let fid = value.get("file_id").and_then(|v| v.as_str()).unwrap_or("");
                                    if !fid.is_empty() {
                                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                                        if let Err(e) = g.insert_pending_tags(fid, &tags) {
                                            eprintln!("[db] insert_pending_tags failed: {e}");
                                        }
                                        applied += 1;
                                    }
                                }
                                done += 1;
                                if done >= handle.total {
                                    Some((Msg::AddonBatchDone { addon_idx, method: "classify".into(), applied, failed }, (handle, conn, name, addon_idx, done, applied, failed)))
                                } else {
                                    Some((Msg::AddonBatchProgress { name: name.clone(), done, total: handle.total }, (handle, conn, name, addon_idx, done, applied, failed)))
                                }
                            }
                            Ok(Ok(Err(e))) => {
                                eprintln!("[addon] classify error: {e}");
                                done += 1;
                                failed += 1;
                                if done >= handle.total {
                                    Some((Msg::AddonBatchDone { addon_idx, method: "classify".into(), applied, failed }, (handle, conn, name, addon_idx, done, applied, failed)))
                                } else {
                                    Some((Msg::AddonBatchProgress { name: name.clone(), done, total: handle.total }, (handle, conn, name, addon_idx, done, applied, failed)))
                                }
                            }
                            _ => {
                                let total = handle.total;
                                let remaining = total.saturating_sub(done);
                                failed += remaining;
                                done = total;
                                Some((Msg::AddonBatchDone { addon_idx, method: "classify".into(), applied, failed }, (handle, conn, name, addon_idx, done, applied, failed)))
                            }
                        }
                    },
                );
                Task::stream(stream)
            }

            Msg::AddonProgress { .. } => Task::none(),

            Msg::AddonBatchProgress { name, done, total } => {
                if total == 100 {
                    self.status = format!("{name}… ({done}%)");
                } else {
                    self.status = format!("{name}… ({done}/{total})");
                }
                Task::none()
            }

            Msg::AddonBatchDone { addon_idx, method, applied, failed } => {
                if failed == 0 {
                    self.status = format!("{method} done — {applied} file{} updated", if applied == 1 { "" } else { "s" });
                    return Task::none();
                }
                let report_path = self.addons.get(addon_idx)
                    .and_then(|addon| write_crash_report(addon, applied, failed));
                self.status = match &report_path {
                    Some(path) => format!("{method} done — {applied} updated, {failed} failed — report: {path}"),
                    None => format!("{method} done — {applied} updated, {failed} failed (addon crashed)"),
                };
                let manifest = self.addons.get(addon_idx).map(|a| a.manifest.clone());
                if let Some(manifest) = manifest {
                    Task::perform(
                        async move { AddonProcess::launch(manifest).map(Arc::new).ok() },
                        move |p| Msg::AddonRestarted { idx: addon_idx, process: p },
                    )
                } else {
                    Task::none()
                }
            }

            Msg::RunFaceClustering => {
                let Some(addon) = self
                    .addons
                    .iter()
                    .find(|a| a.manifest.capabilities.contains(&"cluster_faces".to_string()))
                    .cloned()
                else {
                    self.status = "No face clustering addon installed".to_string();
                    return Task::none();
                };
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                self.status = "Clustering faces… (0%)".to_string();

                let files = {
                    let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                    g.get_all_file_paths_with_mtimes().unwrap_or_default()
                };
                let file_params: Vec<serde_json::Value> = files
                    .iter()
                    .map(|(id, path, mtime)| {
                        serde_json::json!({
                            "file_id": id,
                            "image_path": path,
                            "file_mtime": mtime,
                        })
                    })
                    .collect();
                let params = serde_json::json!({"files": file_params});

                let handle = match addon.send("cluster_faces", params) {
                    Ok(h) => h,
                    Err(e) => {
                        self.status = format!("face clustering error: {e}");
                        return Task::none();
                    }
                };

                let stream = futures::stream::unfold(
                    (handle, conn, false),
                    |(handle, conn, done)| async move {
                        if done { return None; }

                        let handle_result = |conn: &Arc<std::sync::Mutex<isomfolio_core::Catalog>>, result: serde_json::Value| {
                            let clusters = parse_cluster_response(&result);
                            let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                            if let Err(e) = g.save_face_clusters(&clusters) {
                                eprintln!("[db] save_face_clusters failed: {e}");
                            }
                            g.get_face_cluster_summaries().unwrap_or_default()
                        };

                        match handle.progress_rx.recv_timeout(Duration::from_millis(200)) {
                            Ok(percent) => {
                                Some((Msg::AddonBatchProgress { name: "Clustering faces".into(), done: percent as usize, total: 100 }, (handle, conn, false)))
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                match handle.result_rx.try_recv() {
                                    Ok(Ok(result)) => {
                                        let summaries = handle_result(&conn, result);
                                        Some((Msg::FaceClusteringDone(summaries), (handle, conn, true)))
                                    }
                                    Ok(Err(e)) => {
                                        eprintln!("[faces] cluster_faces error: {e}");
                                        Some((Msg::FaceClusteringDone(Vec::new()), (handle, conn, true)))
                                    }
                                    Err(_) => Some((Msg::NoOp, (handle, conn, false)))
                                }
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                match handle.result_rx.recv() {
                                    Ok(Ok(result)) => {
                                        let summaries = handle_result(&conn, result);
                                        Some((Msg::FaceClusteringDone(summaries), (handle, conn, true)))
                                    }
                                    Ok(Err(e)) => {
                                        eprintln!("[faces] cluster_faces error: {e}");
                                        Some((Msg::FaceClusteringDone(Vec::new()), (handle, conn, true)))
                                    }
                                    Err(_) => None,
                                }
                            }
                        }
                    },
                );
                Task::stream(stream)
            }

            Msg::FaceClusteringDone(summaries) => {
                let count = summaries.len();
                self.faces.clusters = summaries;
                self.status = format!("Face clustering done — {count} people found");
                Task::none()
            }

            Msg::FaceClustersLoaded(summaries) => {
                self.faces.clusters = summaries;
                Task::none()
            }

            Msg::RenameFaceCluster(cluster_id) => {
                let current_name = self
                    .faces.clusters
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
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                if let Some(c) = self.faces.clusters.iter_mut().find(|c| c.cluster_id == cluster_id) {
                    c.name = Some(name.clone());
                }
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = g.rename_face_cluster( &cluster_id, &name) {
                            eprintln!("[db] rename_face_cluster failed: {e}");
                        }
                    },
                    |_| Msg::NoOp,
                )
            }

            Msg::MergeFaceClusters(target_id, source_id) => {
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.faces.clusters.retain(|c| c.cluster_id != source_id);
                if let Some(target) = self.faces.clusters.iter_mut().find(|c| c.cluster_id == target_id) {
                    target.file_count += 1;
                }
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
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
                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = g.remove_file_from_face_cluster(&cluster_id, &file_id) {
                            eprintln!("[db] remove_file_from_face_cluster failed: {e}");
                        }
                        g.get_face_cluster_summaries().unwrap_or_default()
                    },
                    Msg::FaceClustersLoaded,
                )
            }

            Msg::AddonRestarted { idx, process } => {
                if let Some(p) = process {
                    if idx < self.addons.len() {
                        self.addons[idx] = p;
                    } else {
                        self.addons.push(p);
                    }
                    self.status = "Addon restarted".to_string();
                } else {
                    self.status = "Addon restart failed — check logs".to_string();
                }
                Task::none()
            }

            Msg::OpenSettings => {
                let mut addon_configs = std::collections::HashMap::new();
                for addon in &self.addons {
                    if addon.manifest.config_schema.is_empty() {
                        continue;
                    }
                    let stored = load_addon_config(&addon.manifest.name);
                    let mut fields = std::collections::HashMap::new();
                    for field in &addon.manifest.config_schema {
                        let val = stored
                            .get(&field.key)
                            .and_then(|v| v.as_str())
                            .unwrap_or(field.default.as_deref().unwrap_or(""))
                            .to_string();
                        fields.insert(field.key.clone(), val);
                    }
                    addon_configs.insert(addon.manifest.name.clone(), fields);
                }
                self.settings = SettingsState { show: true, addon_configs, install_error: None };
                Task::none()
            }

            Msg::CloseSettings => {
                self.settings.show = false;
                Task::none()
            }

            Msg::SettingsConfigChanged { addon_name, key, value } => {
                self.settings
                    .addon_configs
                    .entry(addon_name)
                    .or_default()
                    .insert(key, value);
                Task::none()
            }

            Msg::SaveSettings => {
                self.settings.show = false;
                let mut restart_tasks = Vec::new();
                for (addon_name, fields) in &self.settings.addon_configs {
                    let config: serde_json::Value = fields
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect::<serde_json::Map<_, _>>()
                        .into();
                    if let Err(e) = save_addon_config(addon_name, &config) {
                        self.status = format!("Settings save failed: {e}");
                        return Task::none();
                    }
                    let idx = self.addons.iter().position(|a| &a.manifest.name == addon_name);
                    if let Some(idx) = idx {
                        let manifest = self.addons[idx].manifest.clone();
                        restart_tasks.push(Task::perform(
                            async move { AddonProcess::launch(manifest).map(Arc::new).ok() },
                            move |p| Msg::AddonRestarted { idx, process: p },
                        ));
                    }
                }
                if restart_tasks.is_empty() {
                    Task::none()
                } else {
                    self.status = "Restarting addons…".to_string();
                    Task::batch(restart_tasks)
                }
            }

            Msg::InstallAddonPickFile => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("Folio Addon", &["faddon"])
                        .pick_file()
                        .await
                        .map(|f| f.path().to_string_lossy().into_owned())
                },
                Msg::AddonPackagePicked,
            ),

            Msg::AddonPackagePicked(None) => Task::none(),

            Msg::AddonPackagePicked(Some(path)) => {
                self.settings.install_error = None;
                self.status = "Installing addon…".to_string();
                Task::perform(
                    async move {
                        let path = std::path::PathBuf::from(path);
                        install_addon_package(&path)
                            .and_then(|m| AddonProcess::launch(m).map(Arc::new).map_err(|e| e.to_string()))
                    },
                    |result| match result {
                        Ok(p) => Msg::AddonInstalled(p),
                        Err(e) => Msg::AddonInstallFailed(e),
                    },
                )
            }

            Msg::AddonInstalled(process) => {
                self.status = format!("Addon '{}' installed", process.manifest.name);
                self.settings.install_error = None;
                self.addons.push(process);
                Task::none()
            }

            Msg::AddonInstallFailed(e) => {
                self.settings.install_error = Some(e);
                Task::none()
            }

            Msg::UninstallAddon(name) => {
                if let Some(idx) = self.addons.iter().position(|a| a.manifest.name == name) {
                    self.addons.remove(idx); // kills process via Drop
                }
                if let Err(e) = uninstall_addon(&name) {
                    self.status = format!("Uninstall failed: {e}");
                } else {
                    self.status = format!("Addon '{name}' removed");
                }
                Task::none()
            }

            Msg::SidebarItemClicked(item) => {
                if let SidebarItem::Album(ref id) = item {
                    if let Some(album) = self.albums.iter().find(|a| &a.id == id) {
                        if let AlbumKind::Smart(ref q) = album.kind {
                            self.criteria.tags = q.tags.clone();
                            self.criteria.date_from =
                                q.date_from.map(unix_to_date_str).unwrap_or_default();
                            self.criteria.date_to =
                                q.date_to.map(unix_to_date_str).unwrap_or_default();
                            self.criteria.exts = q.extensions.iter().cloned().collect();
                            self.search_text = q.text.clone().unwrap_or_default();
                            self.criteria.show = true;
                        }
                    }
                }
                self.selected_item = item;
                self.files.clear();
                self.file_ratings.clear();
                self.scroll_y = 0.0;
                self.loupe.idx = 0;
                self.grid_selected.clear();
                self.drag.state = None;
                self.drag.ids.clear();
                self.criteria.save_smart_input = None;
                self.detail.file_id = None;
                self.remove_from_album_pending = false;
                self.smart_album_dirty = false;
                self.load_files_task()
            }

            Msg::FilesLoaded(files) => {
                self.files = files;
                self.enqueue_thumbnails();
                self.status = format!("{} photo(s)", self.files.len());
                let t1 = self.maybe_load_detail();
                let t2 = self.load_ratings_task();
                Task::batch([t1, t2])
            }

            Msg::SidebarLoaded {
                folders,
                albums,
                album_counts,
            } => {
                self.folders = folders;
                self.albums = albums;
                self.album_counts = album_counts;
                self.start_watchers_for_folders();
                if let Some(id) = self.pending_album_select.take() {
                    Task::done(Msg::SidebarItemClicked(SidebarItem::Album(id)))
                } else if self.selected_item == SidebarItem::AllFiles {
                    if let Some((path, _, _)) = self.folders.first() {
                        Task::done(Msg::SidebarItemClicked(SidebarItem::Folder(path.clone())))
                    } else if let Some(album) = self.albums.first() {
                        Task::done(Msg::SidebarItemClicked(SidebarItem::Album(album.id.clone())))
                    } else {
                        self.load_files_task()
                    }
                } else {
                    Task::none()
                }
            }

            Msg::TileSizeUp => {
                self.tile_px = (self.tile_px + 40.0).min(400.0);
                Task::none()
            }

            Msg::TileSizeDown => {
                self.tile_px = (self.tile_px - 40.0).max(80.0);
                Task::none()
            }

            Msg::Navigate { dx, dy } => {
                if matches!(self.view_mode, ViewMode::Loupe) {
                    let total = self.files.len();
                    if total == 0 {
                        return Task::none();
                    }
                    let delta = dx + dy;
                    let new_idx =
                        (self.loupe.idx as i32 + delta).rem_euclid(total as i32) as usize;
                    self.loupe.idx = new_idx;
                    self.loupe.prefetch.retain(|&k, _| {
                        (k as i32 - new_idx as i32).unsigned_abs() as usize <= 2
                    });
                    if let Some(handle) = self.loupe.prefetch.remove(&new_idx) {
                        self.loupe.full_res = Some((new_idx, handle));
                        return self.load_loupe_prefetch();
                    }
                    self.loupe.full_res = None;
                    return Task::batch([self.load_loupe_full_res(), self.load_loupe_prefetch()]);
                }
                let cols = self.cols().max(1) as i32;
                let total = self.files.len() as i32;
                if total == 0 {
                    return Task::none();
                }
                let current = self.anchor_idx.unwrap_or(0) as i32;
                let row = current / cols;
                let col = current % cols;
                let new_col = (col + dx).clamp(0, cols - 1);
                let new_row = (row + dy).clamp(0, (total - 1) / cols);
                let new_idx = (new_row * cols + new_col).min(total - 1) as usize;
                self.anchor_idx = Some(new_idx);
                self.grid_selected.clear();
                if let Some(f) = self.files.get(new_idx) {
                    self.grid_selected.insert(f.id.clone());
                }
                self.maybe_load_detail()
            }

            Msg::OpenLoupe => {
                match self.view_mode {
                    ViewMode::Loupe => {
                        self.view_mode = ViewMode::Browse;
                        self.loupe.full_res = None;
                        self.loupe.prefetch.clear();
                    }
                    ViewMode::Browse => {
                        if !self.files.is_empty() {
                            let idx = self.anchor_idx.unwrap_or(0).min(self.files.len() - 1);
                            self.loupe.idx = idx;
                            self.view_mode = ViewMode::Loupe;
                            if let Some(handle) = self.loupe.prefetch.remove(&idx) {
                                self.loupe.full_res = Some((idx, handle));
                                return self.load_loupe_prefetch();
                            }
                            self.loupe.full_res = None;
                            return Task::batch([self.load_loupe_full_res(), self.load_loupe_prefetch()]);
                        }
                    }
                }
                Task::none()
            }

            Msg::SidebarResizeStart => {
                self.sidebar_resizing = true;
                Task::none()
            }

            Msg::MouseMoved(pos) => {
                self.cursor = pos;
                if self.sidebar_resizing {
                    self.sidebar_width = pos.x.clamp(140.0, 400.0);
                    return Task::none();
                }
                if let Some(ref mut d) = self.drag.state {
                    d.cursor = pos;
                    if !d.active {
                        let dx = pos.x - d.start.x;
                        let dy = pos.y - d.start.y;
                        if (dx * dx + dy * dy).sqrt() > super::DRAG_THRESHOLD {
                            d.active = true;
                            let origin_idx = d.origin_idx;
                            let origin_id = self.files[origin_idx].id.clone();
                            self.drag.ids = if self.grid_selected.contains(&origin_id) {
                                self.grid_selected.clone()
                            } else {
                                [origin_id].into()
                            };
                        }
                    }
                }
                if self.drag.state.as_ref().map_or(false, |d| d.active) {
                    if pos.x < self.sidebar_width + SIDEBAR_HANDLE_WIDTH {
                        let n_folders = self.folders.len();
                        let albums_top =
                            SIDEBAR_ALBUMS_BASE_Y + n_folders as f32 * (ALBUM_ITEM_HEIGHT + 2.0);
                        let y_in_content = pos.y + self.sidebar_scroll_y - albums_top;
                        let row_h = ALBUM_ITEM_HEIGHT + 2.0;
                        self.drag.hover_album = if y_in_content >= 0.0 {
                            let idx = (y_in_content / row_h) as usize;
                            self.albums.get(idx).and_then(|a| {
                                if matches!(a.kind, AlbumKind::Manual) {
                                    Some(a.id.clone())
                                } else {
                                    None
                                }
                            })
                        } else {
                            None
                        };
                    } else {
                        self.drag.hover_album = None;
                    }
                }
                Task::none()
            }

            Msg::MouseRightClicked => {
                let pos = self.cursor;
                if pos.x < self.sidebar_width + SIDEBAR_HANDLE_WIDTH {
                    // Determine which sidebar entity is under cursor via hovered state
                    if let Some(ref entity) = self.hovered_sidebar_entity.clone() {
                        let target = match entity {
                            SidebarItem::Folder(path) => {
                                Some(ContextMenuTarget::Folder(path.clone()))
                            }
                            SidebarItem::Album(id) => {
                                let is_smart = self
                                    .albums
                                    .iter()
                                    .find(|a| &a.id == id)
                                    .map(|a| {
                                        matches!(
                                            a.kind,
                                            isomfolio_core::models::AlbumKind::Smart(_)
                                        )
                                    })
                                    .unwrap_or(false);
                                if is_smart {
                                    Some(ContextMenuTarget::SmartAlbum(id.clone()))
                                } else {
                                    Some(ContextMenuTarget::ManualAlbum(id.clone()))
                                }
                            }
                            SidebarItem::AllFiles | SidebarItem::FaceCluster(_) => None,
                        };
                        if let Some(t) = target {
                            self.context_menu = Some(ContextMenuState {
                                position: pos,
                                target: t,
                                submenu_open: false,
                            });
                        }
                    }
                } else if !self.grid_selected.is_empty() {
                    self.context_menu = Some(ContextMenuState {
                        position: pos,
                        target: ContextMenuTarget::GridTiles,
                        submenu_open: false,
                    });
                } else {
                    self.context_menu = None;
                }
                Task::none()
            }

            Msg::MousePressed => {
                if self.modifiers.control() {
                    return self.update(Msg::MouseRightClicked);
                }
                self.context_menu = None;
                let pos = self.cursor;
                if matches!(self.view_mode, ViewMode::Browse) {
                    if let Some(idx) = self.tile_index_at(pos) {
                        let file_id = self.files[idx].id.clone();
                        let mods = self.modifiers;
                        if mods.command() {
                            if self.grid_selected.contains(&file_id) {
                                self.grid_selected.remove(&file_id);
                            } else {
                                self.grid_selected.insert(file_id.clone());
                                self.anchor_idx = Some(idx);
                            }
                        } else if mods.shift() {
                            let anchor = self.anchor_idx.unwrap_or(idx);
                            let lo = anchor.min(idx);
                            let hi = anchor.max(idx);
                            for i in lo..=hi {
                                if let Some(f) = self.files.get(i) {
                                    self.grid_selected.insert(f.id.clone());
                                }
                            }
                        } else if !self.grid_selected.contains(&file_id) {
                            self.grid_selected.clear();
                            self.grid_selected.insert(file_id);
                            self.anchor_idx = Some(idx);
                        }
                        self.drag.state = Some(DragState {
                            origin_idx: idx,
                            start: pos,
                            cursor: pos,
                            active: false,
                        });
                    } else if pos.x > self.sidebar_width + SIDEBAR_HANDLE_WIDTH {
                        let mods = self.modifiers;
                        if !mods.command() && !mods.shift() {
                            self.grid_selected.clear();
                            self.anchor_idx = None;
                        }
                    }
                }
                if self.detail.show && self.grid_selected.len() != 1 {
                    self.detail.file_id = None;
                    self.detail.rating = None;
                    self.detail.label = None;
                    self.detail.title = None;
                    self.detail.exif_tech = None;
                    if self.grid_selected.is_empty() {
                        self.detail.tags.clear();
                        self.detail.batch_file_ids.clear();
                    }
                }
                Task::none()
            }

            Msg::MouseReleased => {
                if self.sidebar_resizing {
                    self.sidebar_resizing = false;
                    return Task::none();
                }
                let was_drag_active = self.drag.state.as_ref().map_or(false, |d| d.active);
                let drop_task = if was_drag_active {
                    self.drag.hover_album.clone().map(|id| {
                        let ids: Vec<String> = self.drag.ids.iter().cloned().collect();
                        Task::done(Msg::DroppedToAlbum(id, ids))
                    })
                } else {
                    None
                };

                let loupe_task: Option<Task<Msg>> =
                    if !was_drag_active && matches!(self.view_mode, ViewMode::Browse) {
                        if self.tile_index_at(self.cursor).is_some() {
                            if self
                                .last_click_time
                                .map_or(false, |t| t.elapsed().as_millis() < 300)
                            {
                                self.last_click_time = None;
                                Some(Task::done(Msg::OpenLoupe))
                            } else {
                                self.last_click_time = Some(Instant::now());
                                None
                            }
                        } else {
                            self.last_click_time = None;
                            None
                        }
                    } else {
                        self.last_click_time = None;
                        None
                    };

                self.drag.state = None;
                self.drag.ids.clear();
                self.drag.hover_album = None;

                let detail_task = self.maybe_load_detail();
                Task::batch(
                    [drop_task, loupe_task, Some(detail_task)]
                        .into_iter()
                        .flatten(),
                )
            }

            Msg::ModifiersChanged(m) => {
                self.modifiers = m;
                Task::none()
            }

            Msg::EscapePressed => {
                if self.show_shortcut_help {
                    self.show_shortcut_help = false;
                    return Task::none();
                }
                if self.tag_browser.is_some() {
                    self.tag_browser = None;
                    return Task::none();
                }
                if self.context_menu.is_some() {
                    self.context_menu = None;
                    return Task::none();
                }
                if matches!(self.view_mode, ViewMode::Loupe) {
                    self.view_mode = ViewMode::Browse;
                    return Task::none();
                }
                self.create_album_input = None;
                self.rename_album_id = None;
                self.faces.rename_cluster_id = None;
                self.criteria.save_smart_input = None;
                self.remove_from_album_pending = false;
                Task::none()
            }

            Msg::Scrolled { y, height, width } => {
                self.scroll_y = y;
                self.viewport_height = height;
                self.viewport_width = width;
                Task::none()
            }

            Msg::DroppedToAlbum(album_id, ids) => {
                self.drag.state = None;
                self.drag.ids.clear();
                self.drag.hover_album = None;
                let name = self
                    .albums
                    .iter()
                    .find(|a| a.id == album_id)
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                let count = ids.len();
                self.status = format!("Added {count} photo(s) to \"{name}\"");

                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
                        for fid in &ids {
                            if let Err(e) = guard.add_file_to_album( &album_id, fid) {
                                eprintln!("[db] add_file_to_album failed: {e}");
                            }
                        }
                    },
                    |()| Msg::DropCompleted,
                )
            }

            Msg::DropCompleted => self.load_sidebar_task(),

            Msg::ScanPickFolder => {
                if self.is_scanning || self.scan_pending {
                    return Task::none();
                }
                self.scan_pending = true;
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Choose folder to scan")
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_string_lossy().to_string())
                    },
                    Msg::ScanDialogDone,
                )
            }

            Msg::ScanDialogDone(opt) => {
                self.scan_pending = false;
                match opt {
                    None => Task::none(),
                    Some(path) => {
                        if is_under_catalog_dir(&path) {
                            let name = std::path::Path::new(&path)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(path.as_str())
                                .to_string();
                            self.status = format!("\"{}\" is inside a catalog — choose a regular folder", name);
                            return Task::none();
                        }
                        Task::done(Msg::ScanStart(path))
                    }
                }
            }

            Msg::ScanStart(path) => {
                self.last_scanned_path = Some(path.clone());
                self.is_scanning = true;
                self.status = "Scanning…".to_string();
                self.scan_folder_task(path)
            }

            Msg::ScanComplete { count, new_file_ids } => {
                self.is_scanning = false;
                self.status = format!("Scanned {count} photo(s)");
                let path = self.last_scanned_path.take();
                let t1 = self.load_sidebar_task();
                let t_nav = if let Some(p) = path {
                    Task::done(Msg::SidebarItemClicked(SidebarItem::Folder(p)))
                } else {
                    self.load_files_task()
                };
                let has_new = !new_file_ids.is_empty();
                let t_autotag = if has_new {
                    self.auto_tag_task(new_file_ids)
                } else {
                    Task::none()
                };
                let t_faces = if has_new && self.addons.iter().any(|a| a.manifest.capabilities.iter().any(|c| c == "cluster_faces")) {
                    Task::done(Msg::RunFaceClustering)
                } else {
                    Task::none()
                };
                Task::batch([t1, t_nav, t_autotag, t_faces])
            }

            Msg::RequestRemoveFolder(path) => {
                self.folder_pending_remove = Some(path);
                Task::none()
            }

            Msg::CancelRemoveFolder => {
                self.folder_pending_remove = None;
                Task::none()
            }

            Msg::RemoveFolder(path) => {
                self.folder_pending_remove = None;
                self.folders.retain(|(p, _, _)| p != &path);
                self.watchers.retain(|(p, _)| p != &path);
                if self.selected_item == SidebarItem::Folder(path.clone()) {
                    self.selected_item = SidebarItem::AllFiles;
                    self.files.clear();
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = guard.delete_files_by_root_folder( &path) {
                            eprintln!("[db] delete_files_by_root_folder failed: {e}");
                        }
                    },
                    |()| Msg::FolderRemoved,
                )
            }

            Msg::FolderRemoved => {
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::StartCreateAlbum => {
                self.create_album_input = Some(String::new());
                Task::none()
            }

            Msg::CreateAlbumInputChanged(s) => {
                self.create_album_input = Some(s);
                Task::none()
            }

            Msg::ConfirmCreateAlbum => {
                let name = self.create_album_input.take().unwrap_or_default();
                let name = name.trim().to_string();
                if name.is_empty() {
                    return Task::none();
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                let album = Album {
                    id: new_album_id(),
                    name,
                    kind: AlbumKind::Manual,
                    sort_order: 0,
                };
                self.pending_album_select = Some(album.id.clone());
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = guard.create_album( &album) {
                            eprintln!("[db] create_album failed: {e}");
                        }
                    },
                    |()| Msg::AlbumCreated,
                )
            }

            Msg::CancelCreateAlbum => {
                self.create_album_input = None;
                Task::none()
            }

            Msg::AlbumCreated | Msg::AlbumRenamed => {
                self.load_sidebar_task()
            }

            Msg::FilesRemovedFromAlbum => {
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::StartRenameAlbum(album_id) => {
                let current_name = self
                    .albums
                    .iter()
                    .find(|a| a.id == album_id)
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                self.rename_album_id = Some(album_id);
                self.rename_album_input = current_name;
                Task::none()
            }

            Msg::RenameAlbumInputChanged(s) => {
                self.rename_album_input = s;
                Task::none()
            }

            Msg::ConfirmRenameAlbum => {
                let name = self.rename_album_input.trim().to_string();
                let Some(album_id) = self.rename_album_id.take() else {
                    return Task::none();
                };
                if name.is_empty() {
                    return Task::none();
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = guard.rename_album( &album_id, &name) {
                            eprintln!("[db] rename_album failed: {e}");
                        }
                    },
                    |()| Msg::AlbumRenamed,
                )
            }

            Msg::RequestDeleteAlbum(album_id) => {
                self.album_pending_delete = Some(album_id);
                Task::none()
            }

            Msg::CancelDeleteAlbum => {
                self.album_pending_delete = None;
                Task::none()
            }

            Msg::DeleteAlbum(album_id) => {
                self.album_pending_delete = None;
                if self.selected_item == SidebarItem::Album(album_id.clone()) {
                    self.selected_item = SidebarItem::AllFiles;
                    self.files.clear();
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = guard.delete_album( &album_id) {
                            eprintln!("[db] delete_album failed: {e}");
                        }
                    },
                    |()| Msg::AlbumDeleted,
                )
            }

            Msg::AlbumDeleted => {
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::RemoveFromAlbum => {
                self.remove_from_album_pending = true;
                Task::none()
            }

            Msg::CancelRemoveFromAlbum => {
                self.remove_from_album_pending = false;
                Task::none()
            }

            Msg::ConfirmRemoveFromAlbum => {
                self.remove_from_album_pending = false;
                let SidebarItem::Album(ref album_id) = self.selected_item else {
                    return Task::none();
                };
                let album_id = album_id.clone();
                let ids: Vec<String> = self.grid_selected.iter().cloned().collect();
                let count = ids.len();
                let name = self
                    .albums
                    .iter()
                    .find(|a| a.id == album_id)
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                self.status = format!("Removed {count} photo(s) from \"{name}\"");
                self.grid_selected.clear();
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
                        for fid in &ids {
                            if let Err(e) = guard.remove_file_from_album( &album_id, fid) {
                                eprintln!("[db] remove_file_from_album failed: {e}");
                            }
                        }
                    },
                    |()| Msg::FilesRemovedFromAlbum,
                )
            }

            Msg::SortFieldCycle => {
                self.sort_by = next_sort_field(self.sort_by);
                self.load_files_task()
            }

            Msg::SortDirToggle => {
                self.sort_asc = !self.sort_asc;
                self.load_files_task()
            }

            Msg::SortCycleAll => {
                if self.sort_asc {
                    self.sort_asc = false;
                } else {
                    self.sort_by = next_sort_field(self.sort_by);
                    self.sort_asc = true;
                }
                self.load_files_task()
            }

            Msg::SearchChanged(text) => {
                self.mark_smart_dirty();
                self.pending_search = Some((text, Instant::now()));
                Task::none()
            }

            Msg::ToggleCriteria => {
                self.criteria.show = !self.criteria.show;
                Task::none()
            }

            Msg::CriteriaTagInputChanged(s) => {
                self.criteria.tag_input = s;
                Task::none()
            }

            Msg::AddCriteriaTag => {
                let tag = self.criteria.tag_input.trim().to_string();
                self.criteria.tag_input.clear();
                if !tag.is_empty() && !self.criteria.tags.contains(&tag) {
                    self.criteria.tags.push(tag);
                }
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::RemoveCriteriaTag(tag) => {
                self.criteria.tags.retain(|t| t != &tag);
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::CriteriaDateFromChanged(s) => {
                self.criteria.date_from = s;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::CriteriaDateToChanged(s) => {
                self.criteria.date_to = s;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::ToggleCriteriaExt(ext) => {
                if self.criteria.exts.contains(&ext) {
                    self.criteria.exts.remove(&ext);
                } else {
                    self.criteria.exts.insert(ext);
                }
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::ClearCriteria => {
                self.criteria.tags.clear();
                self.criteria.date_from.clear();
                self.criteria.date_to.clear();
                self.criteria.exts.clear();
                self.criteria.flag_filter = isomfolio_core::models::FlagFilter::All;
                self.criteria.rating_min = None;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::SaveAsSmartAlbum => {
                self.criteria.save_smart_input = Some(String::new());
                Task::none()
            }

            Msg::SmartAlbumNameChanged(s) => {
                self.criteria.save_smart_input = Some(s);
                Task::none()
            }

            Msg::ConfirmSmartAlbum => {
                let name = self.criteria.save_smart_input.take().unwrap_or_default();
                let name = name.trim().to_string();
                if name.is_empty() {
                    return Task::none();
                }
                let query = self.build_search_query();
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                let album = Album {
                    id: new_album_id(),
                    name,
                    kind: AlbumKind::Smart(query),
                    sort_order: 0,
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = guard.create_album( &album) {
                            eprintln!("[db] create_album failed: {e}");
                        }
                    },
                    |()| Msg::AlbumCreated,
                )
            }

            Msg::SmartAlbumUpdated => {
                self.smart_album_dirty = false;
                self.load_sidebar_task()
            }

            Msg::UpdateSmartAlbum => {
                let SidebarItem::Album(ref id) = self.selected_item else {
                    return Task::none();
                };
                let album_id = id.clone();
                let query = self.build_search_query();
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = guard.update_smart_album_query( &album_id, &query) {
                            eprintln!("[db] update_smart_album_query failed: {e}");
                        }
                    },
                    |()| Msg::SmartAlbumUpdated,
                )
            }

            Msg::ToggleDetail => {
                self.detail.show = !self.detail.show;
                if self.detail.show {
                    self.detail.file_id = None;
                    self.maybe_load_detail()
                } else {
                    Task::none()
                }
            }

            Msg::DetailLoaded {
                file_id,
                tags,
                tag_origins,
                tag_confidence,
                pending_tags,
                rating,
                label,
                title,
                exif_tech,
            } => {
                self.detail.file_id = Some(file_id);
                self.detail.batch_file_ids.clear();
                self.detail.tags = tags;
                self.detail.tag_origins = tag_origins;
                self.detail.tag_confidence = tag_confidence;
                self.detail.pending_tags = pending_tags;
                self.detail.rating = rating;
                self.detail.label = label;
                self.detail.title = title;
                self.detail.exif_tech = exif_tech;
                self.load_all_tags_task()
            }

            Msg::BatchDetailLoaded { file_ids, tags } => {
                self.detail.file_id = None;
                self.detail.batch_file_ids = file_ids;
                self.detail.tags = tags;
                self.detail.rating = None;
                self.detail.label = None;
                self.detail.title = None;
                self.detail.exif_tech = None;
                self.load_all_tags_task()
            }

            Msg::BatchTagsChanged => {
                self.load_all_tags_task()
            }

            Msg::DetailTagInputChanged(s) => {
                self.detail.tag_input = s;
                Task::none()
            }

            Msg::AddDetailTag => {
                let tag = self.detail.tag_input.trim().to_string();
                self.detail.tag_input.clear();
                if tag.is_empty() || self.detail.tags.contains(&tag) {
                    return Task::none();
                }
                self.detail.tags.push(tag.clone());
                self.detail.push_recent_tag(&tag);
                if !self.detail.batch_file_ids.is_empty() {
                    self.batch_add_tag_task(tag)
                } else {
                    self.save_detail_tags_task()
                }
            }

            Msg::AddDetailTagDirect(tag) => {
                let tag = tag.trim().to_string();
                if tag.is_empty() || self.detail.tags.contains(&tag) {
                    return Task::none();
                }
                self.detail.tags.push(tag.clone());
                self.detail.tag_input.clear();
                self.detail.push_recent_tag(&tag);
                if !self.detail.batch_file_ids.is_empty() {
                    self.batch_add_tag_task(tag)
                } else {
                    self.save_detail_tags_task()
                }
            }

            Msg::RemoveDetailTag(tag) => {
                self.detail.tags.retain(|t| t != &tag);
                if !self.detail.batch_file_ids.is_empty() {
                    self.batch_remove_tag_task(tag)
                } else {
                    self.save_detail_tags_task()
                }
            }

            Msg::AllTagsLoaded(tags) => {
                self.detail.all_tags = tags;
                Task::none()
            }

            Msg::RepeatLastTag => {
                let Some(tag) = self.detail.recent_tags.first().cloned() else {
                    return Task::none();
                };
                self.update(Msg::AddDetailTagDirect(tag))
            }

            Msg::AcceptPendingTag(tag) => {
                let Some(ref fid) = self.detail.file_id else { return Task::none() };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.detail.pending_tags.retain(|(t, _)| t != &tag);
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = g.accept_pending_tag(&fid, &tag) {
                            eprintln!("[db] accept_pending_tag failed: {e}");
                        }
                    },
                    |()| Msg::PendingTagsUpdated,
                )
            }

            Msg::RejectPendingTag(tag) => {
                let Some(ref fid) = self.detail.file_id else { return Task::none() };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.detail.pending_tags.retain(|(t, _)| t != &tag);
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = g.reject_pending_tag(&fid, &tag) {
                            eprintln!("[db] reject_pending_tag failed: {e}");
                        }
                    },
                    |()| Msg::PendingTagsUpdated,
                )
            }

            Msg::AcceptAllPending => {
                let Some(ref fid) = self.detail.file_id else { return Task::none() };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.detail.pending_tags.clear();
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = g.accept_all_pending(&fid) {
                            eprintln!("[db] accept_all_pending failed: {e}");
                        }
                    },
                    |()| Msg::PendingTagsUpdated,
                )
            }

            Msg::RejectAllPending => {
                let Some(ref fid) = self.detail.file_id else { return Task::none() };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.detail.pending_tags.clear();
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = g.reject_all_pending(&fid) {
                            eprintln!("[db] reject_all_pending failed: {e}");
                        }
                    },
                    |()| Msg::PendingTagsUpdated,
                )
            }

            Msg::PendingTagsUpdated => {
                self.detail.file_id = None;
                self.maybe_load_detail()
            }

            Msg::ToggleShortcutHelp => {
                self.show_shortcut_help = !self.show_shortcut_help;
                Task::none()
            }

            Msg::SetDetailRating(n) => {
                let new_rating = if self.detail.rating == Some(n) {
                    None
                } else {
                    Some(n)
                };
                self.detail.rating = new_rating;
                let Some(ref fid) = self.detail.file_id else {
                    return Task::none();
                };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = g.set_file_rating( &fid, new_rating) {
                            eprintln!("[db] set_file_rating failed: {e}");
                        }
                    },
                    |()| Msg::NoOp,
                )
            }

            Msg::SetFlag(flag) => {
                let ids: Vec<String> = if matches!(self.view_mode, ViewMode::Loupe) {
                    self.files.get(self.loupe.idx).map(|f| vec![f.id.clone()]).unwrap_or_default()
                } else {
                    self.grid_selected.iter().cloned().collect()
                };
                if ids.is_empty() {
                    return Task::none();
                }
                for id in &ids {
                    if let Some(f) = self.files.iter_mut().find(|f| &f.id == id) {
                        f.flag = flag;
                    }
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                let flag_clone = flag;
                let ids_clone = ids;
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = g.set_files_flag( &ids_clone, flag_clone) {
                            eprintln!("[db] set_files_flag failed: {e}");
                        }
                    },
                    |()| Msg::FlagsApplied,
                )
            }

            Msg::FlagsApplied => {
                if self.criteria.hide_rejects || self.criteria.flag_filter != isomfolio_core::models::FlagFilter::All {
                    self.load_files_task()
                } else {
                    Task::none()
                }
            }

            Msg::SetRating(rating) => {
                let ids: Vec<String> = if matches!(self.view_mode, ViewMode::Loupe) {
                    self.files.get(self.loupe.idx).map(|f| vec![f.id.clone()]).unwrap_or_default()
                } else {
                    self.grid_selected.iter().cloned().collect()
                };
                if ids.is_empty() {
                    return Task::none();
                }
                for id in &ids {
                    match rating {
                        Some(r) if r > 0 => { self.file_ratings.insert(id.clone(), r); }
                        _ => { self.file_ratings.remove(id); }
                    }
                }
                if ids.len() == 1 && self.detail.file_id.as_deref() == Some(ids[0].as_str()) {
                    self.detail.rating = rating;
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                let ids_clone = ids;
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = g.set_files_rating( &ids_clone, rating) {
                            eprintln!("[db] set_files_rating failed: {e}");
                        }
                    },
                    |()| Msg::RatingsApplied,
                )
            }

            Msg::RatingsApplied => {
                if self.criteria.rating_min.is_some() {
                    self.load_files_task()
                } else {
                    Task::none()
                }
            }

            Msg::RatingsLoaded(map) => {
                self.file_ratings = map;
                Task::none()
            }

            Msg::ToggleHideRejects => {
                self.criteria.hide_rejects = !self.criteria.hide_rejects;
                self.load_files_task()
            }

            Msg::SetFlagFilter(filter) => {
                self.criteria.flag_filter = filter;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::SetRatingFilter(min) => {
                self.criteria.rating_min = min;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::Reload => {
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::Tick => {
                let mut tasks: Vec<Task<Msg>> = Vec::new();
                while let Ok(ev) = self.thumb_ctx.rx.try_recv() {
                    match ev {
                        super::ThumbnailEvent::Ready(fid, path) => {
                            self.thumbnails.insert(
                                fid.clone(),
                                isomfolio_core::models::ThumbnailState::Ready(path.clone()),
                            );
                            self.thumb_ctx.pending = self.thumb_ctx.pending.saturating_sub(1);
                            let fid2 = fid.clone();
                            tasks.push(Task::perform(
                                async move {
                                    tokio::task::spawn_blocking(move || {
                                        image::open(&path).ok().map(|img| {
                                            let rgba = img.into_rgba8();
                                            let (w, h) = (rgba.width(), rgba.height());
                                            (fid2, iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw()))
                                        })
                                    })
                                    .await
                                    .ok()
                                    .flatten()
                                },
                                |res| match res {
                                    Some((fid, handle)) => Msg::ThumbnailHandleReady { file_id: fid, handle },
                                    None => Msg::NoOp,
                                },
                            ));
                        }
                        super::ThumbnailEvent::Failed(fid, _err) => {
                            self.thumbnails
                                .insert(fid, isomfolio_core::models::ThumbnailState::Failed(0));
                            self.thumb_ctx.pending = self.thumb_ctx.pending.saturating_sub(1);
                        }
                    }
                }
                if self.thumb_ctx.pending == 0
                    && self.thumb_ctx.total > 0
                    && self.thumb_ctx.done_at.is_none()
                {
                    self.thumb_ctx.done_at = Some(Instant::now());
                }
                if let Some(done_at) = self.thumb_ctx.done_at {
                    if done_at.elapsed() >= Duration::from_secs(2) {
                        self.thumb_ctx.total = 0;
                        self.thumb_ctx.done_at = None;
                    }
                }

                if let Some((_, ts)) = &self.pending_search {
                    if ts.elapsed() >= Duration::from_millis(300) {
                        let (text, _) = self.pending_search.take().unwrap();
                        self.search_text = text;
                        self.scroll_y = 0.0;
                        self.files.clear();
                        self.grid_selected.clear();
                        tasks.push(self.load_files_task());
                    }
                }

                let mut file_events: Vec<FileEvent> = Vec::new();
                while let Ok(ev) = self.watcher_rx.try_recv() {
                    match ev {
                        FileEvent::ScanProgress(prog) => {
                            self.status = format!(
                                "Scanning {}… {} found",
                                prog.folder_name, prog.total_found
                            );
                        }
                        other => file_events.push(other),
                    }
                }
                if !file_events.is_empty() {
                    if let Some(conn) = self.catalog.clone() {
                        tasks.push(Task::perform(
                            async move {
                                let cat = conn.lock().unwrap_or_else(|e| e.into_inner());
                                for event in file_events {
                                    match event {
                                        FileEvent::Created(path) | FileEvent::Modified(path) => {
                                            let _ = cat.resync_files(&[path]);
                                        }
                                        FileEvent::Deleted(path) => {
                                            let norm = normalize_path(&path);
                                            let fid = compute_file_id(&norm);
                                            if let Err(e) = cat.mark_orphaned(&fid) {
                                                eprintln!("[db] mark_orphaned failed: {e}");
                                            }
                                        }
                                        FileEvent::Renamed { old_path, new_path } => {
                                            let norm = normalize_path(&old_path);
                                            let old_fid = compute_file_id(&norm);
                                            if let Err(e) = cat.mark_orphaned(&old_fid) {
                                                eprintln!("[db] mark_orphaned failed: {e}");
                                            }
                                            let _ = cat.resync_files(&[new_path]);
                                        }
                                        FileEvent::SidecarChanged(path) => {
                                            let _ = cat.resync_sidecar_files(&[path]);
                                        }
                                        FileEvent::SidecarRemoved(_) => {}
                                        FileEvent::ScanProgress(_) => {}
                                    }
                                }
                            },
                            |()| Msg::Reload,
                        ));
                    }
                }

                Task::batch(tasks)
            }

            Msg::DbError(e) => {
                self.status = format!("Error: {e}");
                Task::none()
            }

            Msg::DragHoverAlbum(opt_id) => {
                if self.drag.state.as_ref().map_or(false, |d| d.active) {
                    self.drag.hover_album = opt_id;
                }
                Task::none()
            }

            Msg::PickOpenCatalog => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Open Catalog")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_string_lossy().to_string())
                },
                |opt| match opt {
                    Some(path) => Msg::OpenCatalogPicked(path),
                    None => Msg::NoOp,
                },
            ),

            Msg::SelectRecentCatalog(path) => {
                self.welcome.selected_recent_catalog = Some(path);
                Task::none()
            }

            Msg::OpenSelectedRecentCatalog => {
                let Some(path) = self.welcome.selected_recent_catalog.clone() else {
                    return Task::none();
                };
                Task::done(Msg::OpenCatalog(path))
            }

            Msg::ShowNewCatalogModal => {
                self.welcome.show_new_catalog_modal = true;
                self.welcome.new_catalog_dir = None;
                self.welcome.new_catalog_name.clear();
                Task::none()
            }

            Msg::HideNewCatalogModal => {
                self.welcome.show_new_catalog_modal = false;
                self.welcome.new_catalog_dir = None;
                self.welcome.new_catalog_name.clear();
                Task::none()
            }

            Msg::SidebarScrolled(y) => {
                self.sidebar_scroll_y = y;
                Task::none()
            }

            Msg::OpenCatalogPicked(path) => {
                if !is_catalog_dir(&path) {
                    let name = std::path::Path::new(&path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(path.as_str())
                        .to_string();
                    self.status = format!("\"{}\" is not a valid catalog", name);
                    return Task::none();
                }
                Task::done(Msg::OpenCatalog(path))
            }

            Msg::PickNewCatalogDir => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose location for new catalog")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_string_lossy().to_string())
                },
                |opt| match opt {
                    Some(dir) => Msg::NewCatalogDirPicked(dir),
                    None => Msg::NoOp,
                },
            ),

            Msg::NewCatalogDirPicked(dir) => {
                self.welcome.new_catalog_dir = Some(dir);
                Task::none()
            }

            Msg::NewCatalogNameChanged(s) => {
                self.welcome.new_catalog_name = s;
                Task::none()
            }

            Msg::ConfirmNewCatalog => {
                let Some(dir) = self.welcome.new_catalog_dir.clone() else {
                    return Task::none();
                };
                let name = self.welcome.new_catalog_name.trim().to_string();
                if name.is_empty() {
                    return Task::none();
                }
                self.welcome.show_new_catalog_modal = false;
                Task::perform(
                    async move {
                        isomfolio_core::app_paths::create_catalog(&dir, &name)
                            .map_err(|e| e.to_string())
                    },
                    |result| match result {
                        Ok(path) => Msg::OpenCatalog(path),
                        Err(e) => Msg::DbError(e),
                    },
                )
            }

            Msg::OpenCatalog(path) => {
                isomfolio_core::app_paths::save_recent_catalog(&path);
                self.watchers.clear();
                self.thumb_ctx.pool = None;
                self.thumb_ctx.pending = 0;
                self.thumb_ctx.total = 0;
                self.thumb_ctx.start_at = None;
                self.thumb_ctx.done_at = None;
                self.files.clear();
                self.file_ratings.clear();
                self.thumbnails.clear();
                self.folders.clear();
                self.albums.clear();
                self.album_counts.clear();
                self.grid_selected.clear();
                self.drag.state = None;
                self.drag.ids.clear();
                self.pending_search = None;
                self.search_text.clear();
                self.criteria = CriteriaState::default();
                self.detail = DetailState::default();
                self.selected_item = SidebarItem::AllFiles;
                self.scroll_y = 0.0;
                self.loupe.idx = 0;
                self.view_mode = ViewMode::Browse;
                self.loupe.full_res = None;
                self.loupe.prefetch.clear();
                self.thumb_ctx.handles.clear();
                self.album_pending_delete = None;
                self.folder_pending_remove = None;
                self.welcome.selected_recent_catalog = Some(path.clone());
                self.welcome.show_new_catalog_modal = false;
                self.welcome.new_catalog_dir = None;
                self.welcome.new_catalog_name.clear();
                isomfolio_core::app_paths::ensure_directories(&path);
                self.catalog = isomfolio_core::Catalog::open(&db_path(&path))
                    .ok()
                    .map(|c| std::sync::Arc::new(std::sync::Mutex::new(c)));
                self.status = if self.catalog.is_none() {
                    "Error: could not open database — check permissions".to_string()
                } else {
                    String::new()
                };
                self.catalog_dir = path;
                self.welcome.show = false;
                self.tag_browser = None;
                self.welcome.recent_catalogs = isomfolio_core::app_paths::read_recent_catalogs();
                Task::batch([Task::done(Msg::CatalogReady), App::resize_to_main()])
            }

            Msg::OpenContextMenu(pos, target) => {
                self.context_menu = Some(ContextMenuState {
                    position: pos,
                    target,
                    submenu_open: false,
                });
                Task::none()
            }

            Msg::ToggleAddToAlbumSubmenu => {
                if let Some(ref mut cm) = self.context_menu {
                    cm.submenu_open = !cm.submenu_open;
                }
                Task::none()
            }

            Msg::CloseContextMenu => {
                self.context_menu = None;
                Task::none()
            }

            Msg::HoverSidebarEntityStart(item) => {
                self.hovered_sidebar_entity = Some(item);
                Task::none()
            }

            Msg::HoverSidebarEntityEnd(item) => {
                if self.hovered_sidebar_entity.as_ref() == Some(&item) {
                    self.hovered_sidebar_entity = None;
                }
                Task::none()
            }

            Msg::RescanFolder(path) => {
                self.context_menu = None;
                self.is_scanning = true;
                self.status = "Scanning…".to_string();
                self.scan_folder_task(path)
            }

            Msg::DuplicateAlbum(album_id) => {
                self.context_menu = None;
                let Some(src) = self.albums.iter().find(|a| a.id == album_id).cloned() else {
                    return Task::none();
                };
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                let new_id = new_album_id();
                self.pending_album_select = Some(new_id.clone());
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
                        let new_album = Album {
                            id: new_id.clone(),
                            name: format!("{} copy", src.name),
                            kind: src.kind.clone(),
                            sort_order: 0,
                        };
                        if let Err(e) = guard.create_album( &new_album) {
                            eprintln!("[db] create_album failed: {e}");
                        }
                        if matches!(src.kind, AlbumKind::Manual) {
                            if let Err(e) = guard.copy_album_files( &album_id, &new_id) {
                                eprintln!("[db] copy_album_files failed: {e}");
                            }
                        }
                    },
                    |()| Msg::AlbumCreated,
                )
            }

            Msg::ShowInFinder(path) => {
                self.context_menu = None;
                let _ = std::process::Command::new("open").arg("-R").arg(&path).spawn();
                Task::none()
            }

            Msg::AddSelectionToAlbum(album_id) => {
                self.context_menu = None;
                let ids: Vec<String> = self.grid_selected.iter().cloned().collect();
                let count = ids.len();
                let name = self
                    .albums
                    .iter()
                    .find(|a| a.id == album_id)
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                self.status = format!("Added {count} photo(s) to \"{name}\"");
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
                        for fid in &ids {
                            if let Err(e) = guard.add_file_to_album( &album_id, fid) {
                                eprintln!("[db] add_file_to_album failed: {e}");
                            }
                        }
                    },
                    |()| Msg::DropCompleted,
                )
            }

            Msg::LoupeFullResLoaded { idx, handle } => {
                if self.loupe.idx == idx && matches!(self.view_mode, ViewMode::Loupe) {
                    self.loupe.full_res = Some((idx, handle));
                }
                Task::none()
            }

            Msg::LoupePrefetchLoaded { idx, handle } => {
                if matches!(self.view_mode, ViewMode::Loupe) {
                    let dist = (idx as i32 - self.loupe.idx as i32).unsigned_abs() as usize;
                    if dist <= 2 {
                        self.loupe.prefetch.insert(idx, handle);
                    }
                }
                Task::none()
            }

            Msg::ThumbnailHandleReady { file_id, handle } => {
                self.thumb_ctx.handles.insert(file_id, handle);
                Task::none()
            }

            Msg::OpenTagBrowser
            | Msg::CloseTagBrowser
            | Msg::TagBrowserLoaded(_)
            | Msg::TagBrowserFilterChanged(_)
            | Msg::TagBrowserRenameStart(_)
            | Msg::TagBrowserRenameChanged(_)
            | Msg::TagBrowserRenameConfirm
            | Msg::TagBrowserRenameCancel
            | Msg::TagBrowserDeleteArm(_)
            | Msg::TagBrowserDeleteConfirm
            | Msg::TagBrowserDeleteCancel
            | Msg::TagBrowserTagRenamed
            | Msg::TagBrowserTagDeleted => self.handle_tag_browser(msg),

            Msg::NoOp => Task::none(),
        }
    }
}

impl App {
    fn handle_tag_browser(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::OpenTagBrowser => {
                self.tag_browser = Some(TagBrowserState::default());
                self.load_tag_browser_task()
            }
            Msg::CloseTagBrowser => {
                self.tag_browser = None;
                Task::none()
            }
            Msg::TagBrowserLoaded(tags) => {
                if let Some(ref mut tb) = self.tag_browser {
                    tb.tags = tags;
                }
                Task::none()
            }
            Msg::TagBrowserFilterChanged(s) => {
                if let Some(ref mut tb) = self.tag_browser {
                    tb.filter = s;
                }
                Task::none()
            }
            Msg::TagBrowserRenameStart(tag) => {
                if let Some(ref mut tb) = self.tag_browser {
                    tb.rename = Some((tag.clone(), tag));
                    tb.delete_armed = None;
                }
                Task::none()
            }
            Msg::TagBrowserRenameChanged(s) => {
                if let Some(ref mut tb) = self.tag_browser {
                    if let Some((_, ref mut input)) = tb.rename {
                        *input = s;
                    }
                }
                Task::none()
            }
            Msg::TagBrowserRenameConfirm => {
                let Some(ref tb) = self.tag_browser else {
                    return Task::none();
                };
                let Some((ref old, ref new_name)) = tb.rename else {
                    return Task::none();
                };
                let old = old.clone();
                let new_name = new_name.trim().to_string();
                if new_name.is_empty() || new_name == old {
                    if let Some(ref mut tb) = self.tag_browser {
                        tb.rename = None;
                    }
                    return Task::none();
                }
                if let Some(ref mut tb) = self.tag_browser {
                    tb.rename = None;
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = g.rename_prefixed_tags( &old, &new_name) {
                            eprintln!("[db] rename_prefixed_tags failed: {e}");
                        }
                    },
                    |()| Msg::TagBrowserTagRenamed,
                )
            }
            Msg::TagBrowserRenameCancel => {
                if let Some(ref mut tb) = self.tag_browser {
                    tb.rename = None;
                }
                Task::none()
            }
            Msg::TagBrowserDeleteArm(tag) => {
                if let Some(ref mut tb) = self.tag_browser {
                    tb.delete_armed = Some(tag);
                    tb.rename = None;
                }
                Task::none()
            }
            Msg::TagBrowserDeleteConfirm => {
                let Some(ref tb) = self.tag_browser else {
                    return Task::none();
                };
                let Some(ref tag) = tb.delete_armed else {
                    return Task::none();
                };
                let tag = tag.clone();
                if let Some(ref mut tb) = self.tag_browser {
                    tb.delete_armed = None;
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = g.delete_tag_with_descendants( &tag) {
                            eprintln!("[db] delete_tag_with_descendants failed: {e}");
                        }
                    },
                    |()| Msg::TagBrowserTagDeleted,
                )
            }
            Msg::TagBrowserDeleteCancel => {
                if let Some(ref mut tb) = self.tag_browser {
                    tb.delete_armed = None;
                }
                Task::none()
            }
            Msg::TagBrowserTagRenamed | Msg::TagBrowserTagDeleted => {
                self.detail.file_id = None;
                let t1 = self.load_tag_browser_task();
                let t2 = self.load_all_tags_task();
                let t3 = self.maybe_load_detail();
                Task::batch([t1, t2, t3])
            }
            _ => Task::none(),
        }
    }

    fn mark_smart_dirty(&mut self) {
        if self.current_album_is_smart() {
            self.smart_album_dirty = true;
        }
    }

    fn save_detail_tags_task(&self) -> Task<Msg> {
        let Some(ref fid) = self.detail.file_id else {
            return Task::none();
        };
        let fid = fid.clone();
        let tags = self.detail.tags.clone();
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                if let Err(e) = g.upsert_tags(&fid, &tags) {
                    eprintln!("[db] upsert_tags failed: {e}");
                }
                g.get_all_tags()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(t, _)| t)
                    .collect::<Vec<_>>()
            },
            Msg::AllTagsLoaded,
        )
    }

    fn batch_add_tag_task(&self, tag: String) -> Task<Msg> {
        let file_ids = self.detail.batch_file_ids.clone();
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                if let Err(e) = g.add_tag_to_files(&file_ids, &tag) {
                    eprintln!("[db] add_tag_to_files failed: {e}");
                }
            },
            |()| Msg::BatchTagsChanged,
        )
    }

    fn batch_remove_tag_task(&self, tag: String) -> Task<Msg> {
        let file_ids = self.detail.batch_file_ids.clone();
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock().unwrap_or_else(|e| e.into_inner());
                if let Err(e) = g.remove_tag_from_files(&file_ids, &tag) {
                    eprintln!("[db] remove_tag_from_files failed: {e}");
                }
            },
            |()| Msg::BatchTagsChanged,
        )
    }

    fn scan_folder_task(&self, path: String) -> Task<Msg> {
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        let wtx = self.watcher_tx.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let cat = conn.lock().unwrap_or_else(|e| e.into_inner());
                    cat.scan_folder(&path, &|prog| {
                        let _ = wtx.try_send(FileEvent::ScanProgress(prog));
                    })
                    .map(|r| (r.total_count, r.new_file_ids))
                    .unwrap_or((0, Vec::new()))
                })
                .await
                .unwrap_or((0, Vec::new()))
            },
            |(count, new_file_ids)| Msg::ScanComplete { count, new_file_ids },
        )
    }

}

fn write_crash_report(addon: &AddonProcess, applied: usize, failed: usize) -> Option<String> {
    use isomfolio_core::app_paths::crash_reports_dir;
    let dir = crash_reports_dir();
    let _ = std::fs::create_dir_all(&dir);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("{}-{ts}.txt", addon.manifest.name));

    let stderr_lines = addon.last_stderr();
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0);

    let config = isomfolio_core::addon::load_addon_config(&addon.manifest.name);
    let config_redacted: serde_json::Map<String, serde_json::Value> = config
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| {
            let is_secret = addon.manifest.config_schema.iter().any(|f| {
                f.key == k && matches!(f.kind, isomfolio_core::addon::ConfigFieldKind::Secret)
            });
            if is_secret { (k, serde_json::Value::String("***".into())) } else { (k, v) }
        })
        .collect();

    let mut report = String::new();
    report.push_str(&format!("Addon: {}\n", addon.manifest.name));
    report.push_str(&format!("OS: {} {}\n", std::env::consts::OS, std::env::consts::ARCH));
    report.push_str(&format!("CPU cores: {cores}\n"));
    report.push_str(&format!("Config: {}\n", serde_json::to_string(&config_redacted).unwrap_or_default()));
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

fn extract_scored_tags(result: &serde_json::Value) -> Vec<(String, Option<f32>)> {
    result
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let tag = t.get("tag")?.as_str()?.to_string();
                    let conf = t.get("confidence").and_then(|c| c.as_f64()).map(|c| c as f32);
                    Some((tag, conf))
                })
                .collect()
        })
        .unwrap_or_default()
}

impl App {
    fn auto_tag_task(&self, new_file_ids: Vec<String>) -> Task<Msg> {
        let classify_idx = self
            .addons
            .iter()
            .position(|a| a.manifest.capabilities.iter().any(|c| c == "classify"));
        let Some(addon_idx) = classify_idx else {
            return Task::none();
        };
        Task::done(Msg::RunAddon {
            addon_idx,
            method: "classify".to_string(),
            file_ids: new_file_ids,
        })
    }

    pub(crate) fn load_loupe_full_res(&self) -> Task<Msg> {
        let idx = self.loupe.idx;
        let Some(file) = self.files.get(idx) else {
            return Task::none();
        };
        let path = file.path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    image::open(&path).ok().map(|img| {
                        let rgba = img.into_rgba8();
                        let (w, h) = (rgba.width(), rgba.height());
                        iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw())
                    })
                })
                .await
                .ok()
                .flatten()
            },
            move |handle_opt| match handle_opt {
                Some(handle) => Msg::LoupeFullResLoaded { idx, handle },
                None => Msg::NoOp,
            },
        )
    }

    pub(crate) fn load_loupe_prefetch(&self) -> Task<Msg> {
        let total = self.files.len();
        if total == 0 {
            return Task::none();
        }
        let current = self.loupe.idx;
        let mut tasks = Vec::new();
        for delta in [-1i32, 1] {
            let idx = (current as i32 + delta).rem_euclid(total as i32) as usize;
            if self.loupe.prefetch.contains_key(&idx) {
                continue;
            }
            if self.loupe.full_res.as_ref().map_or(false, |(i, _)| *i == idx) {
                continue;
            }
            if let Some(file) = self.files.get(idx) {
                let path = file.path.clone();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            image::open(&path).ok().map(|img| {
                                let rgba = img.into_rgba8();
                                let (w, h) = (rgba.width(), rgba.height());
                                iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw())
                            })
                        })
                        .await
                        .ok()
                        .flatten()
                    },
                    move |handle_opt| match handle_opt {
                        Some(handle) => Msg::LoupePrefetchLoaded { idx, handle },
                        None => Msg::NoOp,
                    },
                ));
            }
        }
        Task::batch(tasks)
    }
}

fn new_album_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("alb-{nanos:x}-{seq:x}")
}

fn next_sort_field(f: SortField) -> SortField {
    match f {
        SortField::Name => SortField::Date,
        SortField::Date => SortField::Size,
        SortField::Size => SortField::Ext,
        SortField::Ext => SortField::Name,
    }
}

fn parse_cluster_response(v: &serde_json::Value) -> Vec<FaceClusterMember> {
    let Some(clusters) = v.get("clusters").and_then(|c| c.as_array()) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for cluster in clusters {
        let cluster_id = cluster.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if cluster_id.is_empty() {
            continue;
        }
        if let Some(members) = cluster.get("members").and_then(|m| m.as_array()) {
            for member in members {
                let file_id = member.get("file_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if file_id.is_empty() {
                    continue;
                }
                let bbox = member.get("bbox").unwrap_or(&serde_json::Value::Null);
                rows.push(FaceClusterMember {
                    cluster_id: cluster_id.clone(),
                    file_id,
                    bbox_x: bbox.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    bbox_y: bbox.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    bbox_w: bbox.get("w").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    bbox_h: bbox.get("h").and_then(|v| v.as_f64()).unwrap_or(0.0),
                });
            }
        }
    }
    rows
}
