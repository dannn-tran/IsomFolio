use iced::Task;
use isomfolio_core::file_index::compute_file_id;
use isomfolio_core::indexing::types::FileEvent;
use isomfolio_core::path_utils::{is_under_catalog_dir, normalize_path};

use super::LockUnwrap;
use super::super::{App, Msg};

impl App {
    pub(super) fn handle_scan_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
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
                            self.status =
                                format!("\"{}\" is inside a catalog — choose a regular folder", name);
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
                    Task::done(Msg::SidebarItemClicked(super::super::SidebarItem::Folder(p)))
                } else {
                    self.load_files_task()
                };
                let has_new = !new_file_ids.is_empty();
                let t_autotag = if has_new { self.auto_tag_task(new_file_ids) } else { Task::none() };
                let t_faces = if has_new
                    && self
                        .extensions
                        .iter()
                        .any(|a| a.manifest.capabilities.iter().any(|c| c == "cluster_faces"))
                {
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
                if self.selected_item == super::super::SidebarItem::Folder(path.clone()) {
                    self.selected_item = super::super::SidebarItem::AllFiles;
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

            Msg::RescanFolder(path) => {
                self.context_menu = None;
                self.is_scanning = true;
                self.status = "Scanning…".to_string();
                self.scan_folder_task(path)
            }

            Msg::FileWatcherEvent(event) => {
                if let FileEvent::ScanProgress(prog) = event {
                    self.status = format!(
                        "Scanning {}… {} found",
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
                let mut sidecar: Vec<String> = Vec::new();

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
                        FileEvent::SidecarChanged(p) => sidecar.push(p),
                        FileEvent::ScanProgress(_) | FileEvent::SidecarRemoved(_) => {}
                    }
                }

                if upsert.is_empty() && orphan_ids.is_empty() && sidecar.is_empty() {
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
                            if !sidecar.is_empty() {
                                if let Err(e) = cat.resync_sidecar_files(&sidecar) {
                                    eprintln!("[db] resync_sidecar_files failed: {e}");
                                }
                            }
                        })
                        .await
                        .ok();
                    },
                    |()| Msg::Reload,
                )
            }

            _ => Task::none(),
        }
    }

    pub(super) fn scan_folder_task(&self, path: String) -> Task<Msg> {
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
