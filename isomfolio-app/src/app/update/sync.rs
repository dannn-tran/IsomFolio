use std::path::Path;

use iced::Task;
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
                if let Some(ref synced) = path {
                    let sep = std::path::MAIN_SEPARATOR;
                    let prefix = format!("{synced}{sep}");
                    self.dirty_folders
                        .retain(|d| d != synced && !d.starts_with(&prefix));
                    // Reveal the freshly-synced subtree once the sidebar reloads.
                    self.expand_under_path = Some(synced.clone());
                }
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
                let sep = std::path::MAIN_SEPARATOR;
                let prefix = format!("{path}{sep}");
                self.dirty_folders.retain(|d| d != &path && !d.starts_with(&prefix));
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
                        FileEvent::SyncProgress(_) => {}
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

            Msg::RequestMoveRejectsToTrash => {
                self.reject_trash_pending = true;
                Task::none()
            }

            Msg::CancelMoveRejectsToTrash => {
                self.reject_trash_pending = false;
                Task::none()
            }

            Msg::ConfirmMoveRejectsToTrash => {
                self.reject_trash_pending = false;
                let rejects: Vec<(String, String)> = self
                    .files
                    .iter()
                    .filter(|f| f.flag == isomfolio_core::models::Flag::Reject && !f.is_orphaned)
                    .map(|f| (f.id.clone(), f.path.clone()))
                    .collect();
                if rejects.is_empty() {
                    return Task::none();
                }
                let trash_dir = format!("{}{}Trash", self.catalog_dir, std::path::MAIN_SEPARATOR);
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.status = format!("Moving {} reject(s) to Trash…", rejects.len());
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let _ = std::fs::create_dir_all(&trash_dir);
                            let mut moved_ids: Vec<String> = Vec::new();
                            let mut failed = 0usize;
                            for (id, path) in &rejects {
                                match move_to_trash(path, &trash_dir) {
                                    Ok(()) => moved_ids.push(id.clone()),
                                    Err(_) => failed += 1,
                                }
                            }
                            // Trashed files leave the catalog (their ratings/tags go
                            // with them); the originals are recoverable in Trash/.
                            let cat = conn.lock_unwrap();
                            let _ = cat.delete_files(&moved_ids);
                            (moved_ids.len(), failed)
                        })
                        .await
                        .unwrap_or((0, 0))
                    },
                    |(moved, failed)| Msg::RejectsTrashed { moved, failed },
                )
            }

            Msg::RejectsTrashed { moved, failed } => {
                self.status = if failed > 0 {
                    format!("Moved {moved} reject(s) to Trash, {failed} failed")
                } else {
                    format!("Moved {moved} reject(s) to Trash")
                };
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

/// Normalised parent folder of a path — matches the `folder` column stored on
/// files, so it lines up with folder-tree node paths for dirty-marking.
fn parent_folder(path: &str) -> Option<String> {
    let norm = normalize_path(path);
    std::path::Path::new(&norm)
        .parent()
        .and_then(|p| p.to_str())
        .map(|s| s.to_string())
}

/// Move `src` into `trash_dir`, keeping its filename (disambiguating on
/// collision). Falls back to copy+delete across filesystems.
fn move_to_trash(src: &str, trash_dir: &str) -> std::io::Result<()> {
    let name = Path::new(src)
        .file_name()
        .ok_or_else(|| std::io::Error::other(format!("invalid path: {src}")))?;
    let mut dest = Path::new(trash_dir).join(name);
    if dest.exists() {
        let stem = Path::new(src).file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = Path::new(src).extension().and_then(|e| e.to_str());
        for n in 1.. {
            let candidate = match ext {
                Some(e) => format!("{stem} ({n}).{e}"),
                None => format!("{stem} ({n})"),
            };
            dest = Path::new(trash_dir).join(candidate);
            if !dest.exists() {
                break;
            }
        }
    }
    std::fs::rename(src, &dest).or_else(|_| {
        std::fs::copy(src, &dest).and_then(|_| std::fs::remove_file(src))
    })?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_to_trash_moves_then_disambiguates_collision() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("isom_trash_{nanos}"));
        let src = base.join("src");
        let trash = base.join("Trash");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&trash).unwrap();

        let f1 = src.join("a.jpg");
        std::fs::write(&f1, b"one").unwrap();
        move_to_trash(f1.to_str().unwrap(), trash.to_str().unwrap()).unwrap();
        assert!(trash.join("a.jpg").exists());
        assert!(!f1.exists());

        // A second file with the same name lands as "a (1).jpg".
        let f2 = src.join("a.jpg");
        std::fs::write(&f2, b"two").unwrap();
        move_to_trash(f2.to_str().unwrap(), trash.to_str().unwrap()).unwrap();
        assert!(trash.join("a (1).jpg").exists());

        std::fs::remove_dir_all(&base).ok();
    }
}
