use std::path::Path;

use iced::Task;
use isomfolio_core::file_index::compute_file_id;
use isomfolio_core::indexing::types::FileEvent;
use isomfolio_core::path_utils::{normalize_path, CATALOG_EXT};

use super::LockUnwrap;
use super::super::{App, Msg};

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
                self.last_synced_path = Some(path.clone());
                self.is_syncing = true;
                self.task_panel_open = true;
                self.status = "Syncing…".to_string();
                if let Some(conn) = self.catalog.clone() {
                    let p = path.clone();
                    let guard = conn.lock_unwrap();
                    let _ = guard.upsert_library_root(&p, recursive);
                }
                self.sync_folder_task(path, recursive)
            }

            Msg::SyncComplete { count, new_file_ids } => {
                self.is_syncing = false;
                self.status = format!("Synced {count} photo(s)");
                let path = self.last_synced_path.take();
                let t1 = self.load_sidebar_task();
                let t_nav = if let Some(p) = path {
                    Task::done(Msg::SidebarItemClicked(super::super::SidebarItem::Folder(p)))
                } else {
                    self.load_files_task()
                };
                let has_new = !new_file_ids.is_empty();
                let t_faces = if has_new
                    && self.app_settings.auto_face_cluster
                    && self.inference_manifest.is_some()
                {
                    Task::done(Msg::RunFaceClustering { force_full: false })
                } else {
                    Task::none()
                };
                Task::batch([t1, t_nav, t_faces])
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
                Task::perform(
                    async move {
                        let guard = conn.lock_unwrap();
                        let _ = guard.remove_library_root(&path);
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

                let mut upsert: Vec<String> = Vec::new();
                let mut orphan_ids: Vec<String> = Vec::new();

                for event in events {
                    match event {
                        FileEvent::Created(p) | FileEvent::Modified(p) => upsert.push(p),
                        FileEvent::Deleted(p) => {
                            orphan_ids.push(compute_file_id(&normalize_path(&p)));
                        }
                        FileEvent::Renamed { old_path, new_path } => {
                            orphan_ids.push(compute_file_id(&normalize_path(&old_path)));
                            upsert.push(new_path);
                        }
                        FileEvent::SyncProgress(_) => {}
                    }
                }

                if upsert.is_empty() && orphan_ids.is_empty() {
                    return Task::none();
                }

                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let cat = conn.lock_unwrap();
                            if !upsert.is_empty() {
                                if let Err(e) = cat.resync_files(&upsert) {
                                    eprintln!("[db] resync_files failed: {e}");
                                }
                            }
                            if !orphan_ids.is_empty() {
                                if let Err(e) = cat.mark_orphaned_batch(&orphan_ids) {
                                    eprintln!("[db] mark_orphaned_batch failed: {e}");
                                }
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

            _ => Task::none(),
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
                    cat.sync_folder(&path, &|prog| {
                        let _ = wtx.try_send(FileEvent::SyncProgress(prog));
                    }, import_xmp, import_apple, recursive)
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
        let paths: Vec<String> = self
            .grid_selected
            .iter()
            .filter_map(|id| self.files.iter().find(|f| &f.id == id))
            .map(|f| f.path.clone())
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
                let original_path = self
                    .files
                    .iter()
                    .find(|f| f.id == file_id)
                    .map(|f| f.path.clone())
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

            _ => Task::none(),
        }
    }
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
