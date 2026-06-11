use std::path::Path;

use iced::Task;
use isomfolio_core::indexing::types::FileEvent;
use isomfolio_core::path_utils::{normalize_path, CATALOG_EXT};

use super::LockUnwrap;
use super::super::{App, Msg, ViewMode};

impl App {
    pub(super) fn handle_sync_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::SyncPickFolder => {
                if self.is_syncing || self.sync_pending {
                    return Task::none();
                }
                self.sync_pending = true;
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Choose a folder to sync")
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_string_lossy().to_string())
                    },
                    Msg::SyncDialogDone,
                )
            }

            Msg::SyncPickFolderAt(start) => {
                if self.is_syncing || self.sync_pending {
                    return Task::none();
                }
                self.sync_pending = true;
                Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new()
                            .set_title("Choose a folder to sync");
                        if std::path::Path::new(&start).is_dir() {
                            dialog = dialog.set_directory(&start);
                        }
                        dialog
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_string_lossy().to_string())
                    },
                    Msg::SyncDialogDone,
                )
            }

            Msg::SyncDialogDone(opt) => {
                self.sync_pending = false;
                match opt {
                    None => Task::none(),
                    Some(path) => {
                        if is_under_catalog_dir(&path) {
                            let name = std::path::Path::new(&path)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(path.as_str())
                                .to_string();
                            self.status =
                                format!("\"{}\" is inside a catalog — choose a regular folder", name);
                            return Task::none();
                        }
                        let subfolder_count = count_subfolders(&path);
                        if subfolder_count == 0 {
                            // Nothing to recurse into — skip the prompt entirely.
                            return Task::done(Msg::SyncStart { path, recursive: false });
                        }
                        self.add_folder_prompt = Some(super::super::AddFolderPrompt {
                            path,
                            recursive: true,
                            subfolder_count,
                        });
                        Task::none()
                    }
                }
            }

            Msg::AddFolderPromptToggleRecursive => {
                if let Some(p) = self.add_folder_prompt.as_mut() {
                    p.recursive = !p.recursive;
                }
                Task::none()
            }

            Msg::AddFolderCancel => {
                self.add_folder_prompt = None;
                Task::none()
            }

            Msg::AddFolderConfirm => {
                let Some(prompt) = self.add_folder_prompt.take() else {
                    return Task::none();
                };
                Task::done(Msg::SyncStart { path: prompt.path, recursive: prompt.recursive })
            }

            Msg::ToggleFolderExpanded(path) => {
                if !self.expanded_folders.remove(&path) {
                    self.expanded_folders.insert(path);
                }
                Task::none()
            }

            Msg::SyncStart { path, recursive } => {
                // The raw path drives the disk scan; the normalised form is what
                // matches folder-tree node paths, dirty folders and selection
                // (all case-folded). Mixing them up means expand/nav/dirty silently
                // miss when the picker hands back a different casing.
                let norm = normalize_path(&path);
                self.last_synced_path = Some(norm.clone());
                self.is_syncing = true;
                self.task_panel_open = true;
                self.status = "Syncing…".to_string();
                if let Some(conn) = self.catalog.clone() {
                    let p = path.clone();
                    let guard = conn.lock_unwrap();
                    let _ = guard.upsert_library_root(&p, recursive);
                }
                // Reveal the new root in the sidebar now (empty) instead of
                // waiting for the scan to finish indexing files.
                self.expand_under_path = Some(norm);
                Task::batch([self.load_sidebar_task(), self.sync_folder_task(path, recursive)])
            }

            Msg::SyncComplete { count, new_file_ids } => {
                self.is_syncing = false;
                self.status = format!("Synced {count} photo(s)");
                self.bg_mark_done("Sync", format!("Synced {count} photo(s)"));
                let path = self.last_synced_path.take();
                if let Some(ref synced) = path {
                    let sep = std::path::MAIN_SEPARATOR;
                    let prefix = format!("{synced}{sep}");
                    self.dirty_folders
                        .retain(|d| d != synced && !d.starts_with(&prefix));
                    // Reveal the freshly-synced subtree once the sidebar reloads.
                    self.expand_under_path = Some(synced.clone());
                }
                let has_new = !new_file_ids.is_empty();
                // Record this sync's additions as a discrete import batch.
                let t_batch = match (has_new, self.catalog.clone()) {
                    (true, Some(conn)) => {
                        let folder = path.clone();
                        let ids = new_file_ids.clone();
                        Task::perform(
                            async move {
                                let g = conn.lock_unwrap();
                                let _ = g.record_import_batch(folder.as_deref(), &ids);
                                g.get_import_batches(None).unwrap_or_default()
                            },
                            Msg::ImportBatchesLoaded,
                        )
                    }
                    _ => Task::none(),
                };
                let t1 = self.load_sidebar_task();
                let t_nav = if let Some(p) = path {
                    Task::done(Msg::SidebarItemClicked(super::super::SidebarItem::Folder(p)))
                } else {
                    self.load_files_task()
                };
                let t_faces = if has_new
                    && self.app_settings.auto_face_cluster
                    && self.inference_manifest.is_some()
                {
                    Task::done(Msg::RunFaceClustering { force_full: false })
                } else {
                    Task::none()
                };
                // Stack any already-thumbnailed files now; freshly-generated
                // thumbnails get picked up later via the thumbnail-batch drain.
                let t_stack = if has_new && self.app_settings.auto_stack {
                    Task::done(Msg::RunStacking)
                } else {
                    Task::none()
                };
                // Embed scenes from existing thumbnails too; like stacking, freshly
                // generated thumbnails are embedded later via the batch drain.
                let t_scene = if has_new && self.app_settings.auto_scene_embed {
                    Task::done(Msg::RunSceneEmbedding)
                } else {
                    Task::none()
                };
                Task::batch([t_batch, t1, t_nav, t_faces, t_stack, t_scene])
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
                if self.selected_item == super::super::SidebarItem::Folder(path.clone()) {
                    self.selected_item = super::super::SidebarItem::AllFiles;
                    self.files.clear();
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                self.expanded_folders.remove(&path);
                let sep = std::path::MAIN_SEPARATOR;
                let prefix = format!("{path}{sep}");
                self.dirty_folders.retain(|d| d != &path && !d.starts_with(&prefix));
                self.discovered_folders
                    .retain(|d, _| d != &path && !d.starts_with(&prefix));
                let catalog_dir = self.catalog_dir.clone();
                Task::perform(
                    async move {
                        let guard = conn.lock_unwrap();
                        let _ = guard.remove_library_root(&path);
                        let err = guard
                            .delete_files_by_root_folder(&path)
                            .err()
                            .map(|e| e.to_string());
                        // Reclaim the removed files' thumbnails/previews now rather
                        // than waiting for the next catalog open.
                        let _ = guard.sweep_caches(&catalog_dir);
                        err
                    },
                    |e| e.map_or(Msg::FolderRemoved, Msg::DbError),
                )
            }

            Msg::FolderRemoved => {
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::SyncSelectedFolder => {
                if self.is_syncing || self.sync_pending {
                    return Task::none();
                }
                match &self.selected_item {
                    super::super::SidebarItem::Folder(path) => {
                        Task::done(Msg::SyncFolder(path.clone()))
                    }
                    _ => Task::none(),
                }
            }

            Msg::SyncFolder(path) => {
                self.context_menu = None;
                self.is_syncing = true;
                self.task_panel_open = true;
                self.status = "Syncing…".to_string();
                let recursive = self.recursive_for(&path);
                self.sync_folder_task(path, recursive)
            }

            Msg::FileWatcherEvent(event) => {
                if let FileEvent::SyncProgress(prog) = event {
                    self.status = format!(
                        "Syncing {}… {} found",
                        prog.folder_name, prog.total_found
                    );
                    return Task::none();
                }
                if let FileEvent::FoldersDiscovered(folders) = event {
                    // Hold the discovered subfolders in memory and show them now
                    // (expand state was set at SyncStart) — no wait for indexing,
                    // no DB persistence. They solidify once their files index.
                    for (key, display) in folders {
                        self.discovered_folders.insert(key, display);
                    }
                    return self.load_sidebar_task();
                }
                self.pending_file_events.push(event);
                self.watcher_debounce_id += 1;
                let id = self.watcher_debounce_id;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            std::thread::sleep(std::time::Duration::from_millis(300));
                            id
                        })
                        .await
                        .unwrap_or(id)
                    },
                    Msg::FlushFileEvents,
                )
            }

            Msg::FlushFileEvents(id) => {
                if id != self.watcher_debounce_id {
                    return Task::none();
                }
                let events: Vec<FileEvent> = std::mem::take(&mut self.pending_file_events);
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };

                // Content changes of an already-tracked file (same path) are a
                // cache refresh only — re-read derived data + regenerate the
                // thumbnail, never touching user metadata. Structural changes
                // (add / delete / rename) are NOT auto-applied: they mark the
                // folder dirty and the user applies them by syncing. This keeps
                // the catalog transparent and avoids orphaning on transient
                // unmount/move events.
                let mut modified: Vec<String> = Vec::new();
                for event in events {
                    match event {
                        FileEvent::Modified(p) => modified.push(p),
                        FileEvent::Created(p) | FileEvent::Deleted(p) => {
                            if let Some(folder) = parent_folder(&p) {
                                self.dirty_folders.insert(folder);
                            }
                        }
                        FileEvent::Renamed { old_path, new_path } => {
                            if let Some(folder) = parent_folder(&old_path) {
                                self.dirty_folders.insert(folder);
                            }
                            if let Some(folder) = parent_folder(&new_path) {
                                self.dirty_folders.insert(folder);
                            }
                        }
                        FileEvent::SyncProgress(_) | FileEvent::FoldersDiscovered(_) => {}
                    }
                }

                if modified.is_empty() {
                    return Task::none();
                }

                self.refresh_thumbnails(&modified);
                let to_resync = modified;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let cat = conn.lock_unwrap();
                            if let Err(e) = cat.resync_files(&to_resync) {
                                eprintln!("[db] resync_files failed: {e}");
                            }
                        })
                        .await
                        .ok();
                    },
                    |()| Msg::Reload,
                )
            }

            Msg::SyncXmpForSelection => self.import_external_metadata_for_selection(true, false),

            Msg::SyncAppleTagsForSelection => self.import_external_metadata_for_selection(false, true),

            other => {
                debug_assert!(false, "handle_sync_msg received misrouted message: {other:?}");
                Task::none()
            }
        }
    }

    /// Scan depth for re-syncing `path`: an explicit library root uses its
    /// stored setting; anything else (a subfolder, or a pre-roots-table
    /// catalog) re-syncs recursively, matching the historical default.
    fn recursive_for(&self, path: &str) -> bool {
        self.library_roots
            .iter()
            .find(|r| r.path == path)
            .map(|r| r.recursive)
            .unwrap_or(true)
    }

    pub(super) fn sync_folder_task(&mut self, path: String, recursive: bool) -> Task<Msg> {
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        let wtx = self.watcher_tx.clone();
        // Keyword import defaults on (forward-only — never purges existing tags);
        // toggle it in Settings → General. Apple Finder tags only exist on macOS.
        let import_xmp = self.app_settings.import_xmp_tags.unwrap_or(true);
        let import_apple = self
            .app_settings
            .import_apple_tags
            .unwrap_or(cfg!(target_os = "macos"));
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let cat = conn.lock_unwrap();
                    let wtx_dirs = wtx.clone();
                    cat.sync_folder(
                        &path,
                        &|prog| {
                            let _ = wtx.try_send(FileEvent::SyncProgress(prog));
                        },
                        &|dirs| {
                            let _ = wtx_dirs.try_send(FileEvent::FoldersDiscovered(dirs));
                        },
                        import_xmp,
                        import_apple,
                        recursive,
                    )
                    .map(|r| (r.total_count, r.new_file_ids))
                    .unwrap_or((0, Vec::new()))
                })
                .await
                .unwrap_or((0, Vec::new()))
            },
            |(count, new_file_ids)| Msg::SyncComplete { count, new_file_ids },
        )
    }

    fn import_external_metadata_for_selection(&mut self, xmp: bool, apple: bool) -> Task<Msg> {
        // Real-cased disk paths: `import_external_metadata` reads XMP sidecars /
        // Apple Finder tags off disk, which the folded `path` can't open on a
        // case-sensitive volume. (See AssetFile::disk_path.)
        let paths: Vec<String> = self
            .grid_selected
            .iter()
            .filter_map(|id| self.files.iter().find(|f| &f.id == id))
            .map(|f| f.disk_path())
            .collect();
        if paths.is_empty() {
            return Task::none();
        }
        let Some(conn) = self.catalog.clone() else { return Task::none() };
        self.context_menu = None;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let cat = conn.lock_unwrap();
                    if let Err(e) = cat.import_external_metadata(&paths, xmp, apple) {
                        eprintln!("[db] import_external_metadata failed: {e}");
                    }
                })
                .await
                .ok();
            },
            |()| Msg::Reload,
        )
    }
}

impl App {
    pub(super) fn handle_missing_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::RequestRemoveMissing(folder) => {
                self.remove_missing_folder = Some(folder);
                self.context_menu = None;
                Task::none()
            }

            Msg::CancelRemoveMissing => {
                self.remove_missing_folder = None;
                Task::none()
            }

            Msg::DeleteSelection => {
                // Soft-delete (virtual): mark photos deleted. The files on disk are
                // never touched; they move to the Deleted view. In loupe, the target
                // is always the photo on display (the grid selection may lag behind
                // loupe navigation); removing it slides the next photo into place.
                let ids: Vec<String> = if matches!(self.view_mode, ViewMode::Loupe) {
                    self.files.get(self.loupe.idx).map(|f| vec![f.id.clone()]).unwrap_or_default()
                } else {
                    self.grid_selected.iter().cloned().collect()
                };
                if ids.is_empty() {
                    return Task::none();
                }
                self.soft_set_deleted(ids, true)
            }

            Msg::RestoreSelection => {
                let ids: Vec<String> = self.grid_selected.iter().cloned().collect();
                if ids.is_empty() {
                    return Task::none();
                }
                self.soft_set_deleted(ids, false)
            }

            Msg::RequestDeleteRejects => {
                self.reject_delete_pending = true;
                Task::none()
            }

            Msg::CancelDeleteRejects => {
                self.reject_delete_pending = false;
                Task::none()
            }

            Msg::ConfirmDeleteRejects => {
                self.reject_delete_pending = false;
                let ids: Vec<String> = self
                    .files
                    .iter()
                    .filter(|f| f.flag == isomfolio_core::models::Flag::Reject)
                    .map(|f| f.id.clone())
                    .collect();
                if ids.is_empty() {
                    return Task::none();
                }
                self.soft_set_deleted(ids, true)
            }

            Msg::SelectionDeleted => {
                // Status was set synchronously in `soft_set_deleted`; just refresh
                // the sidebar (Deleted count) and the current view.
                Task::batch([self.load_sidebar_task(), self.load_files_task()])
            }

            Msg::RequestPurgeSelected => {
                // Real-cased `disk_path()`, not the folded `path` — the latter only
                // resolves on case-insensitive volumes, so trashing a file on an
                // external case-sensitive drive would fail. (See AssetFile::disk_path.)
                let targets: Vec<(String, String)> = self
                    .files
                    .iter()
                    .filter(|f| self.grid_selected.contains(&f.id))
                    .map(|f| (f.id.clone(), f.disk_path()))
                    .collect();
                if !targets.is_empty() {
                    self.purge_pending = Some(targets);
                    self.context_menu = None;
                }
                Task::none()
            }

            Msg::RequestPurgeAll => {
                let targets: Vec<(String, String)> =
                    self.files.iter().map(|f| (f.id.clone(), f.disk_path())).collect();
                if !targets.is_empty() {
                    self.purge_pending = Some(targets);
                }
                Task::none()
            }

            Msg::CancelPurge => {
                self.purge_pending = None;
                Task::none()
            }

            Msg::ConfirmPurge => {
                let Some(targets) = self.purge_pending.take() else { return Task::none() };
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                let count = targets.len();
                self.grid_selected.clear();
                let trash_name = crate::app::os_trash_name();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            // The one delete path that touches disk: move files to the
                            // OS Trash / Recycle Bin (recoverable there), then drop the
                            // catalog rows. If the trash move fails, keep the catalog
                            // rows so the photos don't silently vanish while the files
                            // are still on disk — surface the error instead.
                            let paths: Vec<String> =
                                targets.iter().map(|(_, p)| p.clone()).collect();
                            trash::delete_all(&paths).map_err(|e| e.to_string())?;
                            let ids: Vec<String> = targets.into_iter().map(|(id, _)| id).collect();
                            conn.lock_unwrap().delete_files(&ids).map_err(|e| e.to_string())?;
                            Ok::<usize, String>(count)
                        })
                        .await
                        .unwrap_or_else(|e| Err(e.to_string()))
                    },
                    move |res| match res {
                        Ok(n) => Msg::Purged(n),
                        Err(e) => Msg::DbError(format!("Move to {trash_name} failed: {e}")),
                    },
                )
            }

            Msg::Purged(count) => {
                self.status = format!("Moved {count} photo(s) to {}", crate::app::os_trash_name());
                Task::batch([self.load_sidebar_task(), self.load_files_task()])
            }

            Msg::ConfirmRemoveMissing => {
                let Some(folder) = self.remove_missing_folder.take() else {
                    return Task::none();
                };
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let cat = conn.lock_unwrap();
                            cat.purge_orphans_in_folder(&folder).unwrap_or(0)
                        })
                        .await
                        .unwrap_or(0)
                    },
                    |_| Msg::Reload,
                )
            }

            Msg::LocateFile(file_id) => {
                // Real-cased path so the picker opens in the file's actual folder
                // (the folded path's parent may not exist on a case-sensitive volume).
                let original_path = self
                    .files
                    .iter()
                    .find(|f| f.id == file_id)
                    .map(|f| f.disk_path())
                    .unwrap_or_default();
                let start_dir = std::path::Path::new(&original_path)
                    .parent()
                    .map(|p| p.to_path_buf());
                self.context_menu = None;
                Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new()
                            .add_filter("Image", &["jpg", "jpeg", "png", "webp", "gif"]);
                        if let Some(dir) = start_dir {
                            dialog = dialog.set_directory(dir);
                        }
                        dialog.pick_file().await.map(|h| h.path().to_path_buf())
                    },
                    move |opt| match opt {
                        Some(path) => Msg::FileLocated { file_id: file_id.clone(), new_path: path },
                        None => Msg::NoOp,
                    },
                )
            }

            Msg::FileLocated { file_id, new_path } => {
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                let path_str = new_path.to_string_lossy().into_owned();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            conn.lock_unwrap().relocate_file(&file_id, &path_str)
                        })
                        .await
                        .ok()
                        .and_then(|r| r.err())
                        .map(|e| e.to_string())
                    },
                    |err| match err {
                        Some(e) => Msg::DbError(e),
                        None => Msg::Reload,
                    },
                )
            }

            other => {
                debug_assert!(false, "handle_missing_msg received misrouted message: {other:?}");
                Task::none()
            }
        }
    }

    /// Flip the virtual `is_deleted` flag on `ids` (no disk I/O), set a status,
    /// and reload. `deleted = true` moves to the Deleted view; `false` restores.
    fn soft_set_deleted(&mut self, ids: Vec<String>, deleted: bool) -> Task<Msg> {
        let count = ids.len();
        // Grid: after the reload, re-select the photo that slides into the first
        // removed slot (loupe handles its own advance via `loupe_resync`). Without
        // this, delete leaves nothing selected and a stale anchor.
        if !matches!(self.view_mode, ViewMode::Loupe) {
            self.pending_restore_idx = self.files.iter().position(|f| ids.contains(&f.id));
        }
        self.grid_selected.clear();
        self.status = if deleted {
            // First-ever soft-delete: spell out that this is virtual and the files
            // on disk are untouched, then never repeat it (persisted flag).
            if !self.app_settings.seen_delete_hint {
                self.app_settings.seen_delete_hint = true;
                isomfolio_core::app_paths::save_settings(&self.app_settings);
                format!("Moved {count} to Deleted — your files on disk are untouched")
            } else {
                format!("Moved {count} photo(s) to Deleted")
            }
        } else {
            format!("Restored {count} photo(s)")
        };
        self.push_undo(crate::app::UndoOp::SetDeleted { ids: ids.clone(), deleted });
        let Some(conn) = self.catalog.clone() else { return Task::none() };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let _ = conn.lock_unwrap().set_files_deleted(&ids, deleted);
                })
                .await
                .ok();
            },
            |()| Msg::SelectionDeleted,
        )
    }
}

/// Normalised parent folder of a path — matches the `folder` column stored on
/// files, so it lines up with folder-tree node paths for dirty-marking.
fn parent_folder(path: &str) -> Option<String> {
    let norm = normalize_path(path);
    std::path::Path::new(&norm)
        .parent()
        .and_then(|p| p.to_str())
        .map(|s| s.to_string())
}

fn is_under_catalog_dir(path: &str) -> bool {
    std::path::Path::new(path)
        .components()
        .any(|c| Path::new(c.as_os_str()).extension().map_or(false, |ext| ext == CATALOG_EXT))
}

/// Count immediate subdirectories of `path` (ignoring nested catalogs), so the
/// add-folder prompt can tell the user how many subfolders "include subfolders"
/// will pull in.
fn count_subfolders(path: &str) -> usize {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| {
            e.file_name()
                .to_str()
                .map_or(true, |n| !n.ends_with(&format!(".{CATALOG_EXT}")))
        })
        .count()
}

