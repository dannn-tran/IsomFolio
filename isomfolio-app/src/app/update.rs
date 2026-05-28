use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use iced::Task;
use iced::futures;

use isomfolio_core::extension::{
    discover_extensions, install_extension_package, load_extension_config, save_extension_config,
    uninstall_extension, ExtensionProcess,
};
use isomfolio_core::app_paths::extensions_dir;
use isomfolio_core::indexing::thumbnail::thumbnail_cache_path;
use isomfolio_core::models::ThumbnailState;
use isomfolio_core::file_index::compute_file_id;
use isomfolio_core::indexing::types::FileEvent;
use isomfolio_core::models::{Album, AlbumKind, FaceClusterMember, SortField};
use isomfolio_core::path_utils::{is_catalog_dir, is_under_catalog_dir, normalize_path};

use super::{
    unix_to_date_str, App, CompareState, ContextMenuState, ContextMenuTarget, CriteriaState,
    DetailState, DragState, LoupeState, Msg, SettingsState, SidebarItem, TagBrowserState, UndoOp,
    ViewMode, ALBUM_ITEM_HEIGHT, SIDEBAR_ALBUMS_BASE_Y, SIDEBAR_HANDLE_WIDTH,
};
use isomfolio_core::app_paths::db_path;

trait LockUnwrap<T> {
    fn lock_unwrap(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> LockUnwrap<T> for std::sync::Mutex<T> {
    fn lock_unwrap(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl App {
    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::CatalogReady => {
                self.start_thumbnail_pool();
                let sidebar_task = self.load_sidebar_task();
                let addon_task = Task::perform(
                    async move {
                        let dir = extensions_dir();
                        let manifests = discover_extensions(&dir);
                        manifests
                            .into_iter()
                            .filter_map(|m| {
                                ExtensionProcess::launch(m)
                                    .map(Arc::new)
                                    .map_err(|e| eprintln!("[addon] launch failed: {e}"))
                                    .ok()
                            })
                            .collect::<Vec<_>>()
                    },
                    Msg::ExtensionsDiscovered,
                );
                let face_task = if let Some(conn) = self.catalog.clone() {
                    Task::perform(
                        async move {
                            let g = conn.lock_unwrap();
                            g.get_face_cluster_summaries().unwrap_or_default()
                        },
                        Msg::FaceClustersLoaded,
                    )
                } else {
                    Task::none()
                };
                let orphan_task = if let Some(conn) = self.catalog.clone() {
                    Task::perform(
                        async move {
                            let g = conn.lock_unwrap();
                            g.purge_old_orphans(30).ok();
                        },
                        |()| Msg::NoOp,
                    )
                } else {
                    Task::none()
                };
                Task::batch([sidebar_task, addon_task, face_task, orphan_task])
            }

            Msg::ExtensionsDiscovered(addons) => {
                let count = addons.len();
                self.extensions = addons;
                if count > 0 {
                    self.status = format!("{count} addon{} loaded", if count == 1 { "" } else { "s" });
                }
                Task::none()
            }

            Msg::RunExtension { addon_idx, method, file_ids } => {
                let Some(addon) = self.extensions.get(addon_idx).cloned() else {
                    return Task::none();
                };
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                let catalog_dir = self.catalog_dir.clone();
                let total = file_ids.len();
                let extension_name = addon.manifest.name.clone();
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

                let handle = match addon.send_many(&requests) {
                    Ok(h) => h,
                    Err(e) => {
                        self.status = format!("addon error: {e}");
                        return Task::none();
                    }
                };

                let stream = futures::stream::unfold(
                    (handle, conn, extension_name, addon_idx, 0usize, 0usize, 0usize),
                    |(handle, conn, name, addon_idx, mut done, mut applied, mut failed)| async move {
                        let rx = handle.rx.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            rx.lock_unwrap().recv()
                        }).await;
                        match result {
                            Ok(Ok(Ok(value))) => {
                                let fid = value.get("file_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let tags = extract_scored_tags(value);
                                if !tags.is_empty() && !fid.is_empty() {
                                    let g = conn.lock_unwrap();
                                    if let Err(e) = g.insert_pending_tags(&fid, &tags) {
                                        eprintln!("[db] insert_pending_tags failed: {e}");
                                    }
                                    applied += 1;
                                }
                                done += 1;
                                if done >= handle.total {
                                    Some((Msg::ExtensionBatchDone { addon_idx, method: "classify".into(), applied, failed }, (handle, conn, name, addon_idx, done, applied, failed)))
                                } else {
                                    Some((Msg::ExtensionBatchProgress { name: name.clone(), done, total: handle.total }, (handle, conn, name, addon_idx, done, applied, failed)))
                                }
                            }
                            Ok(Ok(Err(e))) => {
                                eprintln!("[addon] classify error: {e}");
                                done += 1;
                                failed += 1;
                                if done >= handle.total {
                                    Some((Msg::ExtensionBatchDone { addon_idx, method: "classify".into(), applied, failed }, (handle, conn, name, addon_idx, done, applied, failed)))
                                } else {
                                    Some((Msg::ExtensionBatchProgress { name: name.clone(), done, total: handle.total }, (handle, conn, name, addon_idx, done, applied, failed)))
                                }
                            }
                            _ => {
                                let total = handle.total;
                                let remaining = total.saturating_sub(done);
                                failed += remaining;
                                done = total;
                                Some((Msg::ExtensionBatchDone { addon_idx, method: "classify".into(), applied, failed }, (handle, conn, name, addon_idx, done, applied, failed)))
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
                    self.status = format!("{method} done — {applied} file{} updated", if applied == 1 { "" } else { "s" });
                    return Task::none();
                }
                let report_path = self.extensions.get(addon_idx)
                    .and_then(|addon| write_crash_report(addon, applied, failed));
                self.status = match &report_path {
                    Some(path) => format!("{method} done — {applied} updated, {failed} failed — report: {path}"),
                    None => format!("{method} done — {applied} updated, {failed} failed (addon crashed)"),
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
                let Some(addon) = self
                    .extensions
                    .iter()
                    .find(|a| a.manifest.capabilities.contains(&"cluster_faces".to_string()))
                    .cloned()
                else {
                    self.faces.status = Some("No face clustering addon installed".to_string());
                    return Task::none();
                };
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                self.faces.status = Some("Clustering faces… (0%)".to_string());

                let files = {
                    let g = conn.lock_unwrap();
                    g.get_all_file_paths_with_mtimes().unwrap_or_default()
                };
                let params = cluster_faces_request_params(&files, force_full);

                let handle = match addon.send("cluster_faces", params) {
                    Ok(h) => h,
                    Err(e) => {
                        self.faces.status = Some(format!("face clustering error: {e}"));
                        return Task::none();
                    }
                };

                let stream = futures::stream::unfold(
                    (handle, conn, false),
                    |(handle, conn, done)| async move {
                        if done { return None; }

                        let handle_result = |conn: &Arc<std::sync::Mutex<isomfolio_core::Catalog>>, result: serde_json::Value| {
                            let clusters = parse_cluster_response(result);
                            let g = conn.lock_unwrap();
                            if let Err(e) = g.save_face_clusters(&clusters) {
                                eprintln!("[db] save_face_clusters failed: {e}");
                            }
                            g.get_face_cluster_summaries().unwrap_or_default()
                        };

                        match handle.progress_rx.recv_timeout(Duration::from_millis(200)) {
                            Ok(percent) => {
                                Some((Msg::ExtensionBatchProgress { name: "Clustering faces".into(), done: percent as usize, total: 100 }, (handle, conn, false)))
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
                self.loupe = LoupeState::default();
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
                        let g = conn.lock_unwrap();
                        g.rename_face_cluster(&cluster_id, &name).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::NoOp, Msg::DbError),
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
                    "Addon restarted".to_string()
                } else {
                    "Addon restart failed — check logs".to_string()
                };
                if self.settings.show {
                    self.settings.status = Some(msg);
                } else {
                    self.status = msg;
                }
                Task::none()
            }

            Msg::OpenSettings
            | Msg::CloseSettings
            | Msg::SettingsConfigChanged { .. }
            | Msg::SaveSettings
            | Msg::InstallExtensionPickFile
            | Msg::ExtensionPackagePicked(_)
            | Msg::ExtensionInstalled(_)
            | Msg::ExtensionInstallFailed(_)
            | Msg::UninstallExtension(_)
            | Msg::SetPreferredExtension { .. } => self.handle_settings(msg),

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
                if matches!(self.view_mode, ViewMode::People) {
                    self.view_mode = ViewMode::Browse;
                }
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
                if let Some(idx) = self.anchor_idx {
                    self.scroll_to_index(idx)
                } else {
                    Task::none()
                }
            }

            Msg::TileSizeDown => {
                self.tile_px = (self.tile_px - 40.0).max(80.0);
                if let Some(idx) = self.anchor_idx {
                    self.scroll_to_index(idx)
                } else {
                    Task::none()
                }
            }

            Msg::Navigate { dx, dy } => {
                if matches!(self.view_mode, ViewMode::Loupe | ViewMode::Preview) {
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
                    if matches!(self.view_mode, ViewMode::Preview) {
                        self.anchor_idx = Some(new_idx);
                        self.grid_selected.clear();
                        if let Some(f) = self.files.get(new_idx) {
                            self.grid_selected.insert(f.id.clone());
                        }
                    }
                    let mut tasks = vec![self.load_loupe_full_res(), self.load_loupe_prefetch()];
                    if matches!(self.view_mode, ViewMode::Preview) {
                        tasks.push(self.scroll_to_index(new_idx));
                        tasks.push(self.maybe_load_detail());
                    }
                    if let Some(handle) = self.loupe.prefetch.remove(&new_idx) {
                        self.loupe.full_res = Some((new_idx, handle));
                        return Task::batch(tasks);
                    }
                    self.loupe.full_res = None;
                    return Task::batch(tasks);
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
                let scroll = self.scroll_to_index(new_idx);
                let detail = self.maybe_load_detail();
                Task::batch([scroll, detail])
            }

            Msg::OpenLoupe => {
                match self.view_mode {
                    ViewMode::Loupe => {
                        self.anchor_idx = Some(self.loupe.idx);
                        self.grid_selected.clear();
                        if let Some(f) = self.files.get(self.loupe.idx) {
                            self.grid_selected.insert(f.id.clone());
                        }
                        self.view_mode = ViewMode::Browse;
                        self.loupe.full_res = None;
                        self.loupe.prefetch.clear();
                        return self.scroll_to_index(self.loupe.idx);
                    }
                    ViewMode::Preview => {
                        self.view_mode = ViewMode::Loupe;
                        return Task::none();
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
                    ViewMode::People | ViewMode::Compare => {}
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
                    self.detail.tags.clear();
                    self.detail.batch_file_ids.clear();
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
                if self.open_menu.is_some() {
                    self.open_menu = None;
                    return Task::none();
                }
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
                if matches!(self.view_mode, ViewMode::Compare | ViewMode::Loupe) {
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
                        let guard = conn.lock_unwrap();
                        let failed = ids.iter()
                            .filter(|fid| guard.add_file_to_album(&album_id, fid).is_err())
                            .count();
                        (count, failed)
                    },
                    |(total, failed)| {
                        if failed > 0 {
                            Msg::DbError(format!("{} added, {failed} failed to add to album", total - failed))
                        } else {
                            Msg::DropCompleted
                        }
                    },
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
                let t_faces = if has_new && self.extensions.iter().any(|a| a.manifest.capabilities.iter().any(|c| c == "cluster_faces")) {
                    Task::done(Msg::RunFaceClustering { force_full: false })
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
                        let guard = conn.lock_unwrap();
                        guard.delete_files_by_root_folder(&path).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::FolderRemoved, Msg::DbError),
                )
            }

            Msg::FolderRemoved => {
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::StartCreateAlbum
            | Msg::CreateAlbumInputChanged(_)
            | Msg::ConfirmCreateAlbum
            | Msg::CancelCreateAlbum
            | Msg::AlbumCreated
            | Msg::AlbumRenamed
            | Msg::FilesRemovedFromAlbum
            | Msg::StartRenameAlbum(_)
            | Msg::RenameAlbumInputChanged(_)
            | Msg::ConfirmRenameAlbum
            | Msg::RequestDeleteAlbum(_)
            | Msg::CancelDeleteAlbum
            | Msg::DeleteAlbum(_)
            | Msg::AlbumDeleted
            | Msg::RemoveFromAlbum
            | Msg::CancelRemoveFromAlbum
            | Msg::ConfirmRemoveFromAlbum
            | Msg::SaveAsSmartAlbum
            | Msg::SmartAlbumNameChanged(_)
            | Msg::ConfirmSmartAlbum
            | Msg::SmartAlbumUpdated
            | Msg::UpdateSmartAlbum => self.handle_album(msg),

            Msg::SortFieldCycle
            | Msg::SortDirToggle
            | Msg::SortCycleAll
            | Msg::SearchChanged(_)
            | Msg::ToggleCriteria
            | Msg::CriteriaTagInputChanged(_)
            | Msg::AddCriteriaTag
            | Msg::RemoveCriteriaTag(_)
            | Msg::CriteriaDateFromChanged(_)
            | Msg::CriteriaDateToChanged(_)
            | Msg::ToggleCriteriaExt(_)
            | Msg::ClearCriteria => self.handle_criteria(msg),

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
                let file_ids = self.current_detail_file_ids();
                self.undo_stack.push(UndoOp::AddedTag { file_ids, tag: tag.clone() });
                self.redo_stack.clear();
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
                let file_ids = self.current_detail_file_ids();
                self.undo_stack.push(UndoOp::AddedTag { file_ids, tag: tag.clone() });
                self.redo_stack.clear();
                if !self.detail.batch_file_ids.is_empty() {
                    self.batch_add_tag_task(tag)
                } else {
                    self.save_detail_tags_task()
                }
            }

            Msg::RemoveDetailTag(tag) => {
                self.detail.tags.retain(|t| t != &tag);
                let file_ids = self.current_detail_file_ids();
                self.undo_stack.push(UndoOp::RemovedTag { file_ids, tag: tag.clone() });
                self.redo_stack.clear();
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

            Msg::TagsSavedResult(tags, err) => {
                self.detail.all_tags = tags;
                if let Some(e) = err {
                    self.status = format!("Error saving tags: {e}");
                }
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
                        let g = conn.lock_unwrap();
                        g.accept_pending_tag(&fid, &tag).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::PendingTagsUpdated, Msg::DbError),
                )
            }

            Msg::RejectPendingTag(tag) => {
                let Some(ref fid) = self.detail.file_id else { return Task::none() };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.detail.pending_tags.retain(|(t, _)| t != &tag);
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.reject_pending_tag(&fid, &tag).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::PendingTagsUpdated, Msg::DbError),
                )
            }

            Msg::AcceptAllPending => {
                let Some(ref fid) = self.detail.file_id else { return Task::none() };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.detail.pending_tags.clear();
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.accept_all_pending(&fid).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::PendingTagsUpdated, Msg::DbError),
                )
            }

            Msg::RejectAllPending => {
                let Some(ref fid) = self.detail.file_id else { return Task::none() };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.detail.pending_tags.clear();
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.reject_all_pending(&fid).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::PendingTagsUpdated, Msg::DbError),
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

            Msg::OpenMenuDropdown(name) => {
                self.open_menu = if self.open_menu.as_deref() == Some(name.as_str()) {
                    None
                } else {
                    Some(name)
                };
                Task::none()
            }

            Msg::CloseMenuDropdown => {
                self.open_menu = None;
                Task::none()
            }

            Msg::TogglePreview => {
                match self.view_mode {
                    ViewMode::Preview => {
                        self.view_mode = ViewMode::Browse;
                        self.loupe = LoupeState::default();
                    }
                    ViewMode::Browse => {
                        if let Some(idx) = self.anchor_idx {
                            self.loupe.idx = idx;
                            self.view_mode = ViewMode::Preview;
                            return Task::batch([self.load_loupe_full_res(), self.load_loupe_prefetch()]);
                        }
                    }
                    _ => {}
                }
                Task::none()
            }

            Msg::ReturnToWelcome => {
                self.open_menu = None;
                self.welcome.show = true;
                self.welcome.recent_catalogs = isomfolio_core::app_paths::read_recent_catalogs();
                self.welcome.selected_recent_catalog = None;
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
                        let g = conn.lock_unwrap();
                        g.set_file_rating(&fid, new_rating).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::NoOp, Msg::DbError),
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
                let before: Vec<(String, isomfolio_core::models::Flag)> = ids.iter()
                    .filter_map(|id| self.files.iter().find(|f| &f.id == id).map(|f| (id.clone(), f.flag)))
                    .collect();
                for id in &ids {
                    if let Some(f) = self.files.iter_mut().find(|f| &f.id == id) {
                        f.flag = flag;
                    }
                }
                self.undo_stack.push(UndoOp::SetFlags { before });
                self.redo_stack.clear();
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                let flag_clone = flag;
                let ids_clone = ids;
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.set_files_flag(&ids_clone, flag_clone).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::FlagsApplied, Msg::DbError),
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
                let before: Vec<(String, Option<i32>)> = ids.iter()
                    .map(|id| (id.clone(), self.file_ratings.get(id).copied()))
                    .collect();
                for id in &ids {
                    match rating {
                        Some(r) if r > 0 => { self.file_ratings.insert(id.clone(), r); }
                        _ => { self.file_ratings.remove(id); }
                    }
                }
                if ids.len() == 1 && self.detail.file_id.as_deref() == Some(ids[0].as_str()) {
                    self.detail.rating = rating;
                }
                self.undo_stack.push(UndoOp::SetRatings { before });
                self.redo_stack.clear();
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                let ids_clone = ids;
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.set_files_rating(&ids_clone, rating).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::RatingsApplied, Msg::DbError),
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

            Msg::ThumbnailCompleted { file_id, path } => {
                self.thumbnails.insert(file_id.clone(), ThumbnailState::Ready(path.clone()));
                self.thumb_ctx.pending = self.thumb_ctx.pending.saturating_sub(1);
                let clear_task = if self.thumb_ctx.pending == 0 && self.thumb_ctx.total > 0 {
                    let gen = self.thumb_ctx.done_gen;
                    Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                std::thread::sleep(std::time::Duration::from_secs(2));
                                gen
                            }).await.unwrap_or(gen)
                        },
                        Msg::ClearThumbnailProgress,
                    )
                } else {
                    Task::none()
                };
                let fid2 = file_id.clone();
                let load_task = Task::perform(
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
                );
                Task::batch([load_task, clear_task])
            }

            Msg::ThumbnailFailed { file_id } => {
                self.thumbnails.insert(file_id, ThumbnailState::Failed(0));
                self.thumb_ctx.pending = self.thumb_ctx.pending.saturating_sub(1);
                if self.thumb_ctx.pending == 0 && self.thumb_ctx.total > 0 {
                    let gen = self.thumb_ctx.done_gen;
                    Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                std::thread::sleep(std::time::Duration::from_secs(2));
                                gen
                            }).await.unwrap_or(gen)
                        },
                        Msg::ClearThumbnailProgress,
                    )
                } else {
                    Task::none()
                }
            }

            Msg::FileWatcherEvent(event) => {
                match event {
                    FileEvent::ScanProgress(prog) => {
                        self.status = format!("Scanning {}… {} found", prog.folder_name, prog.total_found);
                        Task::none()
                    }
                    FileEvent::Created(path) | FileEvent::Modified(path) => {
                        let Some(conn) = self.catalog.clone() else { return Task::none(); };
                        Task::perform(
                            async move { let cat = conn.lock_unwrap(); let _ = cat.resync_files(&[path]); },
                            |()| Msg::Reload,
                        )
                    }
                    FileEvent::Deleted(path) => {
                        let Some(conn) = self.catalog.clone() else { return Task::none(); };
                        Task::perform(
                            async move {
                                let cat = conn.lock_unwrap();
                                let norm = normalize_path(&path);
                                let fid = compute_file_id(&norm);
                                if let Err(e) = cat.mark_orphaned(&fid) {
                                    eprintln!("[db] mark_orphaned failed: {e}");
                                }
                            },
                            |()| Msg::Reload,
                        )
                    }
                    FileEvent::Renamed { old_path, new_path } => {
                        let Some(conn) = self.catalog.clone() else { return Task::none(); };
                        Task::perform(
                            async move {
                                let cat = conn.lock_unwrap();
                                let norm = normalize_path(&old_path);
                                let old_fid = compute_file_id(&norm);
                                if let Err(e) = cat.mark_orphaned(&old_fid) {
                                    eprintln!("[db] mark_orphaned failed: {e}");
                                }
                                let _ = cat.resync_files(&[new_path]);
                            },
                            |()| Msg::Reload,
                        )
                    }
                    FileEvent::SidecarChanged(path) => {
                        let Some(conn) = self.catalog.clone() else { return Task::none(); };
                        Task::perform(
                            async move { let cat = conn.lock_unwrap(); let _ = cat.resync_sidecar_files(&[path]); },
                            |()| Msg::Reload,
                        )
                    }
                    FileEvent::SidecarRemoved(_) => Task::none(),
                }
            }

            Msg::ClearThumbnailProgress(gen) => {
                if gen == self.thumb_ctx.done_gen {
                    self.thumb_ctx.total = 0;
                }
                Task::none()
            }

            Msg::SearchDebounceTimer { id, text } => {
                if id != self.search_debounce_id {
                    return Task::none();
                }
                self.search_text = text;
                self.scroll_y = 0.0;
                self.files.clear();
                self.grid_selected.clear();
                self.load_files_task()
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

            Msg::PickOpenCatalog => {
                self.open_menu = None;
                Task::perform(
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
            )}

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
                self.open_menu = None;
                if !self.welcome.show {
                    self.welcome.show = true;
                    self.welcome.recent_catalogs = isomfolio_core::app_paths::read_recent_catalogs();
                }
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
                let (new_tx, new_rx) = mpsc::sync_channel::<super::ThumbnailEvent>(500);
                self.thumb_ctx.tx = new_tx;
                self.thumb_ctx.rx = Arc::new(std::sync::Mutex::new(Some(new_rx)));
                self.thumb_ctx.sub_id += 1;
                self.thumb_ctx.pending = 0;
                self.thumb_ctx.total = 0;
                self.thumb_ctx.start_at = None;
                self.thumb_ctx.done_gen += 1;
                self.files.clear();
                self.file_ratings.clear();
                self.thumbnails.clear();
                self.folders.clear();
                self.albums.clear();
                self.album_counts.clear();
                self.grid_selected.clear();
                self.drag.state = None;
                self.drag.ids.clear();
                self.search_debounce_id += 1;
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

            Msg::OpenFaceClusterMenu(cluster_id) => {
                self.context_menu = Some(ContextMenuState {
                    position: self.cursor,
                    target: ContextMenuTarget::FaceCluster(cluster_id),
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
                        let guard = conn.lock_unwrap();
                        let new_album = Album {
                            id: new_id.clone(),
                            name: format!("{} copy", src.name),
                            kind: src.kind.clone(),
                            sort_order: 0,
                        };
                        let e1 = guard.create_album(&new_album).err();
                        let e2 = if matches!(src.kind, AlbumKind::Manual) {
                            guard.copy_album_files(&album_id, &new_id).err()
                        } else {
                            None
                        };
                        e1.or(e2).map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::AlbumCreated, Msg::DbError),
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
                        let guard = conn.lock_unwrap();
                        let failed = ids.iter()
                            .filter(|fid| guard.add_file_to_album(&album_id, fid).is_err())
                            .count();
                        (count, failed)
                    },
                    |(total, failed)| {
                        if failed > 0 {
                            Msg::DbError(format!("{} added, {failed} failed to add to album", total - failed))
                        } else {
                            Msg::DropCompleted
                        }
                    },
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

            Msg::SelectAll => {
                self.grid_selected = self.files.iter().map(|f| f.id.clone()).collect();
                if self.anchor_idx.is_none() && !self.files.is_empty() {
                    self.anchor_idx = Some(0);
                }
                Task::none()
            }

            Msg::DeselectAll => {
                self.grid_selected.clear();
                self.anchor_idx = None;
                Task::none()
            }

            Msg::Undo => self.apply_undo_op(true),
            Msg::Redo => self.apply_undo_op(false),
            Msg::UndoApplied => {
                let t1 = self.load_files_task();
                let t2 = self.maybe_load_detail();
                let t3 = self.load_ratings_task();
                Task::batch([t1, t2, t3])
            }

            Msg::OpenCompare => {
                if self.grid_selected.len() != 2 {
                    self.status = "Select exactly 2 photos to compare".to_string();
                    return Task::none();
                }
                let mut sel = self.grid_selected.iter();
                let id0 = sel.next().unwrap().clone();
                let id1 = sel.next().unwrap().clone();
                let f0 = self.files.iter().find(|f| f.id == id0).cloned();
                let f1 = self.files.iter().find(|f| f.id == id1).cloned();
                self.compare = CompareState { files: [f0, f1], handles: [None, None] };
                self.view_mode = ViewMode::Compare;
                Task::batch([self.load_compare_slot(0), self.load_compare_slot(1)])
            }

            Msg::CompareFullResLoaded { slot, handle } => {
                if matches!(self.view_mode, ViewMode::Compare) {
                    self.compare.handles[slot] = Some(handle);
                }
                Task::none()
            }

            Msg::NoOp => Task::none(),
        }
    }
}

impl App {
    fn current_detail_file_ids(&self) -> Vec<String> {
        if !self.detail.batch_file_ids.is_empty() {
            self.detail.batch_file_ids.clone()
        } else if let Some(ref fid) = self.detail.file_id {
            vec![fid.clone()]
        } else {
            self.grid_selected.iter().cloned().collect()
        }
    }

    fn apply_undo_op(&mut self, is_undo: bool) -> Task<Msg> {
        let op = if is_undo {
            self.undo_stack.pop()
        } else {
            self.redo_stack.pop()
        };
        let Some(op) = op else { return Task::none() };
        let Some(conn) = self.catalog.clone() else { return Task::none() };

        match op {
            UndoOp::AddedTag { file_ids, tag } => {
                let inverse = UndoOp::RemovedTag { file_ids: file_ids.clone(), tag: tag.clone() };
                if is_undo { self.redo_stack.push(inverse); } else { self.undo_stack.push(inverse); }
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.remove_tag_from_files(&file_ids, &tag).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::UndoApplied, Msg::DbError),
                )
            }
            UndoOp::RemovedTag { file_ids, tag } => {
                let inverse = UndoOp::AddedTag { file_ids: file_ids.clone(), tag: tag.clone() };
                if is_undo { self.redo_stack.push(inverse); } else { self.undo_stack.push(inverse); }
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.add_tag_to_files(&file_ids, &tag).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::UndoApplied, Msg::DbError),
                )
            }
            UndoOp::SetRatings { before } => {
                let after: Vec<(String, Option<i32>)> = before.iter()
                    .map(|(id, _)| (id.clone(), self.file_ratings.get(id).copied()))
                    .collect();
                let inverse = UndoOp::SetRatings { before: after };
                if is_undo { self.redo_stack.push(inverse); } else { self.undo_stack.push(inverse); }
                for (id, rating) in &before {
                    match rating {
                        Some(r) if *r > 0 => { self.file_ratings.insert(id.clone(), *r); }
                        _ => { self.file_ratings.remove(id); }
                    }
                }
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        for (id, rating) in &before {
                            if let Err(e) = g.set_file_rating(id, *rating) {
                                return Some(e.to_string());
                            }
                        }
                        None
                    },
                    |e| e.map_or(Msg::UndoApplied, Msg::DbError),
                )
            }
            UndoOp::SetFlags { before } => {
                let after: Vec<(String, isomfolio_core::models::Flag)> = before.iter()
                    .filter_map(|(id, _)| self.files.iter().find(|f| &f.id == id).map(|f| (id.clone(), f.flag)))
                    .collect();
                let inverse = UndoOp::SetFlags { before: after };
                if is_undo { self.redo_stack.push(inverse); } else { self.undo_stack.push(inverse); }
                for (id, flag) in &before {
                    if let Some(f) = self.files.iter_mut().find(|f| &f.id == id) {
                        f.flag = *flag;
                    }
                }
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        for (id, flag) in &before {
                            if let Err(e) = g.set_file_flag(id, *flag) {
                                return Some(e.to_string());
                            }
                        }
                        None
                    },
                    |e| e.map_or(Msg::UndoApplied, Msg::DbError),
                )
            }
        }
    }

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
                        let g = conn.lock_unwrap();
                        g.rename_prefixed_tags(&old, &new_name).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::TagBrowserTagRenamed, Msg::DbError),
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
                        let g = conn.lock_unwrap();
                        g.delete_tag_with_descendants(&tag).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::TagBrowserTagDeleted, Msg::DbError),
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
                let g = conn.lock_unwrap();
                let err = g.upsert_tags(&fid, &tags).err().map(|e| e.to_string());
                let all_tags = g.get_all_tags()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(t, _)| t)
                    .collect::<Vec<_>>();
                (all_tags, err)
            },
            |(tags, err)| Msg::TagsSavedResult(tags, err),
        )
    }

    fn batch_add_tag_task(&self, tag: String) -> Task<Msg> {
        let file_ids = self.detail.batch_file_ids.clone();
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                g.add_tag_to_files(&file_ids, &tag).err().map(|e| e.to_string())
            },
            |e| e.map_or(Msg::BatchTagsChanged, Msg::DbError),
        )
    }

    fn batch_remove_tag_task(&self, tag: String) -> Task<Msg> {
        let file_ids = self.detail.batch_file_ids.clone();
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                g.remove_tag_from_files(&file_ids, &tag).err().map(|e| e.to_string())
            },
            |e| e.map_or(Msg::BatchTagsChanged, Msg::DbError),
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
                    let cat = conn.lock_unwrap();
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

fn write_crash_report(addon: &ExtensionProcess, applied: usize, failed: usize) -> Option<String> {
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

    let addon_dir = addon.manifest.executable.parent().unwrap_or(std::path::Path::new("."));
    let config = isomfolio_core::extension::load_extension_config(addon_dir);
    let config_redacted: serde_json::Map<String, serde_json::Value> = config
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| {
            let is_secret = addon.manifest.config_schema.iter().any(|f| {
                f.key == k && matches!(f.kind, isomfolio_core::extension::ConfigFieldKind::Secret)
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

fn generate_face_crops(
    catalog_dir: &str,
    reps: &[(String, String, f64, f64, f64, f64)],
) -> Vec<(String, String)> {
    use isomfolio_core::app_paths::face_crop_path;
    let crop_dir = isomfolio_core::app_paths::face_crop_dir(catalog_dir);
    let _ = std::fs::create_dir_all(&crop_dir);

    let mut results = Vec::new();
    for (cluster_id, file_path, bx, by, bw, bh) in reps {
        let out_path = face_crop_path(catalog_dir, cluster_id);
        if std::path::Path::new(&out_path).exists() {
            results.push((cluster_id.clone(), out_path));
            continue;
        }
        let Ok(img) = image::open(file_path) else { continue };
        let (iw, ih) = (img.width() as f64, img.height() as f64);
        let x = (bx * iw).max(0.0) as u32;
        let y = (by * ih).max(0.0) as u32;
        let w = (bw * iw).min(iw - x as f64) as u32;
        let h = (bh * ih).min(ih - y as f64) as u32;
        if w == 0 || h == 0 { continue; }
        let cropped = img.crop_imm(x, y, w, h);
        let thumb = cropped.resize_exact(96, 96, image::imageops::FilterType::Triangle);
        if thumb.save(&out_path).is_ok() {
            results.push((cluster_id.clone(), out_path));
        }
    }
    results
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

fn extract_scored_tags(result: serde_json::Value) -> Vec<(String, Option<f32>)> {
    let Ok(resp) = serde_json::from_value::<ClassifyResponse>(result) else {
        return Vec::new();
    };
    resp.tags.into_iter().map(|t| (t.tag, t.confidence)).collect()
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

impl App {
    fn load_face_crops_task(&self) -> Task<Msg> {
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
                            let handle = iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw());
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

    fn auto_tag_task(&self, new_file_ids: Vec<String>) -> Task<Msg> {
        let preferred = self.prefs.preferred_extension.get("classify").map(|s| s.as_str());
        let classify_idx = self.extensions.iter().position(|a| {
            a.manifest.capabilities.iter().any(|c| c == "classify")
                && preferred.map_or(true, |p| a.manifest.name == p)
        }).or_else(|| {
            // Fall back to first capable addon if preference no longer installed
            self.extensions.iter().position(|a| a.manifest.capabilities.iter().any(|c| c == "classify"))
        });
        let Some(addon_idx) = classify_idx else {
            return Task::none();
        };
        Task::done(Msg::RunExtension {
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

    pub(crate) fn load_compare_slot(&self, slot: usize) -> Task<Msg> {
        let Some(file) = self.compare.files[slot].as_ref() else {
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
                Some(handle) => Msg::CompareFullResLoaded { slot, handle },
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

impl App {
    fn handle_album(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
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
                        let guard = conn.lock_unwrap();
                        guard.create_album(&album).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::AlbumCreated, Msg::DbError),
                )
            }

            Msg::CancelCreateAlbum => {
                self.create_album_input = None;
                Task::none()
            }

            Msg::AlbumCreated | Msg::AlbumRenamed => self.load_sidebar_task(),

            Msg::FilesRemovedFromAlbum => {
                Task::batch([self.load_sidebar_task(), self.load_files_task()])
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
                        let guard = conn.lock_unwrap();
                        guard.rename_album(&album_id, &name).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::AlbumRenamed, Msg::DbError),
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
                        let guard = conn.lock_unwrap();
                        guard.delete_album(&album_id).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::AlbumDeleted, Msg::DbError),
                )
            }

            Msg::AlbumDeleted => {
                Task::batch([self.load_sidebar_task(), self.load_files_task()])
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
                        let guard = conn.lock_unwrap();
                        let err = ids
                            .iter()
                            .find_map(|fid| guard.remove_file_from_album(&album_id, fid).err())
                            .map(|e| e.to_string());
                        err
                    },
                    |e| e.map_or(Msg::FilesRemovedFromAlbum, Msg::DbError),
                )
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
                        let guard = conn.lock_unwrap();
                        guard.create_album(&album).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::AlbumCreated, Msg::DbError),
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
                        let guard = conn.lock_unwrap();
                        guard.update_smart_album_query(&album_id, &query).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::SmartAlbumUpdated, Msg::DbError),
                )
            }

            _ => Task::none(),
        }
    }

    fn handle_criteria(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
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
                self.search_debounce_id += 1;
                let id = self.search_debounce_id;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            std::thread::sleep(std::time::Duration::from_millis(300));
                            (id, text)
                        }).await.unwrap_or((id, String::new()))
                    },
                    |(id, text)| Msg::SearchDebounceTimer { id, text },
                )
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

            _ => Task::none(),
        }
    }
}

impl App {
    fn handle_settings(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::OpenSettings => {
                let mut addon_configs = std::collections::HashMap::new();
                for addon in &self.extensions {
                    if addon.manifest.config_schema.is_empty() {
                        continue;
                    }
                    let addon_dir = addon.manifest.executable.parent().unwrap_or(std::path::Path::new("."));
                    let stored = load_extension_config(addon_dir);
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
                self.settings = SettingsState { show: true, addon_configs, install_error: None, status: None };
                Task::none()
            }

            Msg::CloseSettings => {
                self.settings.show = false;
                Task::none()
            }

            Msg::SettingsConfigChanged { extension_name, key, value } => {
                use isomfolio_core::extension::ConfigFieldKind;
                let kind = self.extensions.iter()
                    .find(|a| a.manifest.name == extension_name)
                    .and_then(|a| a.manifest.config_schema.iter().find(|f| f.key == key))
                    .map(|f| &f.kind);
                let valid = match kind {
                    Some(ConfigFieldKind::Number) => value.is_empty() || value == "." || value == "-" || value.parse::<f64>().is_ok(),
                    Some(ConfigFieldKind::Integer) => value.is_empty() || value == "-" || value.parse::<i64>().is_ok(),
                    _ => true,
                };
                if valid {
                    self.settings
                        .addon_configs
                        .entry(extension_name)
                        .or_default()
                        .insert(key, value);
                }
                Task::none()
            }

            Msg::SaveSettings => {
                self.settings.show = false;
                let mut restart_tasks = Vec::new();
                for (extension_name, fields) in &self.settings.addon_configs {
                    let schema = self.extensions.iter()
                        .find(|a| &a.manifest.name == extension_name)
                        .map(|a| &a.manifest.config_schema);
                    let config: serde_json::Value = fields
                        .iter()
                        .map(|(k, v)| {
                            use isomfolio_core::extension::ConfigFieldKind;
                            let kind = schema.and_then(|s| s.iter().find(|f| &f.key == k)).map(|f| &f.kind);
                            let val = match kind {
                                Some(ConfigFieldKind::Number) => v.parse::<f64>()
                                    .map(serde_json::Value::from)
                                    .unwrap_or_else(|_| serde_json::Value::String(v.clone())),
                                Some(ConfigFieldKind::Integer) => v.parse::<i64>()
                                    .map(serde_json::Value::from)
                                    .unwrap_or_else(|_| serde_json::Value::String(v.clone())),
                                _ => serde_json::Value::String(v.clone()),
                            };
                            (k.clone(), val)
                        })
                        .collect::<serde_json::Map<_, _>>()
                        .into();
                    let addon_dir = self.extensions.iter()
                        .find(|a| &a.manifest.name == extension_name)
                        .and_then(|a| a.manifest.executable.parent().map(|p| p.to_path_buf()))
                        .unwrap_or_default();
                    if let Err(e) = save_extension_config(&addon_dir, &config) {
                        self.status = format!("Settings save failed: {e}");
                        return Task::none();
                    }
                    let idx = self.extensions.iter().position(|a| &a.manifest.name == extension_name);
                    if let Some(idx) = idx {
                        let manifest = self.extensions[idx].manifest.clone();
                        restart_tasks.push(Task::perform(
                            async move { ExtensionProcess::launch(manifest).map(Arc::new).ok() },
                            move |p| Msg::ExtensionRestarted { idx, process: p },
                        ));
                    }
                }
                if restart_tasks.is_empty() {
                    Task::none()
                } else {
                    self.settings.status = Some("Saving & restarting addons…".to_string());
                    Task::batch(restart_tasks)
                }
            }

            Msg::InstallExtensionPickFile => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("IsomFolio Extension", &["isfx"])
                        .pick_file()
                        .await
                        .map(|f| f.path().to_string_lossy().into_owned())
                },
                Msg::ExtensionPackagePicked,
            ),

            Msg::ExtensionPackagePicked(None) => Task::none(),

            Msg::ExtensionPackagePicked(Some(path)) => {
                self.settings.install_error = None;
                self.settings.status = Some("Installing addon…".to_string());
                Task::perform(
                    async move {
                        let path = std::path::PathBuf::from(path);
                        install_extension_package(&path)
                            .and_then(|m| ExtensionProcess::launch(m).map(Arc::new).map_err(|e| e.to_string()))
                    },
                    |result| match result {
                        Ok(p) => Msg::ExtensionInstalled(p),
                        Err(e) => Msg::ExtensionInstallFailed(e),
                    },
                )
            }

            Msg::ExtensionInstalled(process) => {
                self.settings.status = Some(format!("'{}' installed", process.manifest.name));
                self.settings.install_error = None;
                self.extensions.push(process);
                Task::none()
            }

            Msg::ExtensionInstallFailed(e) => {
                self.settings.install_error = Some(e);
                Task::none()
            }

            Msg::UninstallExtension(name) => {
                if let Some(idx) = self.extensions.iter().position(|a| a.manifest.name == name) {
                    self.extensions.remove(idx);
                }
                // Remove any capability preferences that pointed to this addon
                self.prefs.preferred_extension.retain(|_, v| v != &name);
                if let Err(e) = uninstall_extension(&name) {
                    self.settings.status = Some(format!("Uninstall failed: {e}"));
                } else {
                    self.settings.status = Some(format!("'{name}' removed"));
                }
                isomfolio_core::app_paths::save_prefs(&self.prefs);
                Task::none()
            }

            Msg::SetPreferredExtension { capability, extension_name } => {
                self.prefs.preferred_extension.insert(capability, extension_name);
                isomfolio_core::app_paths::save_prefs(&self.prefs);
                Task::none()
            }

            _ => Task::none(),
        }
    }
}
