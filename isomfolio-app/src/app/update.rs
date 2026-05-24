use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use iced::Task;

use isomfolio_core::file_index::compute_file_id;
use isomfolio_core::indexing::scanner;
use isomfolio_core::indexing::types::FileEvent;
use isomfolio_core::models::{Album, AlbumKind, SortField};
use isomfolio_core::path_utils::{is_catalog_dir, is_under_catalog_dir, normalize_path};
use isomfolio_core::storage::db;

use super::{
    unix_to_date_str, App, ContextMenuState, ContextMenuTarget, CriteriaState, DetailState,
    DragState, Msg, SidebarItem, TagBrowserState, ViewMode, ALBUM_ITEM_HEIGHT,
    SIDEBAR_ALBUMS_BASE_Y, SIDEBAR_WIDTH,
};
use isomfolio_core::app_paths::db_path;

impl App {
    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::CatalogReady => {
                self.start_thumbnail_pool();
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
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
                self.scroll_y = 0.0;
                self.loupe_idx = 0;
                self.grid_selected.clear();
                self.drag = None;
                self.dragging_ids.clear();
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
                self.maybe_load_detail()
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
                    self.loupe_idx =
                        (self.loupe_idx as i32 + delta).rem_euclid(total as i32) as usize;
                    self.loupe_full_res = None;
                    return self.load_loupe_full_res();
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
                        self.loupe_full_res = None;
                    }
                    ViewMode::Browse => {
                        if !self.files.is_empty() {
                            self.loupe_idx =
                                self.anchor_idx.unwrap_or(0).min(self.files.len() - 1);
                            self.view_mode = ViewMode::Loupe;
                            self.loupe_full_res = None;
                            return self.load_loupe_full_res();
                        }
                    }
                }
                Task::none()
            }

            Msg::MouseMoved(pos) => {
                self.cursor = pos;
                if let Some(ref mut d) = self.drag {
                    d.cursor = pos;
                    if !d.active {
                        let dx = pos.x - d.start.x;
                        let dy = pos.y - d.start.y;
                        if (dx * dx + dy * dy).sqrt() > super::DRAG_THRESHOLD {
                            d.active = true;
                            let origin_idx = d.origin_idx;
                            let origin_id = self.files[origin_idx].id.clone();
                            self.dragging_ids = if self.grid_selected.contains(&origin_id) {
                                self.grid_selected.clone()
                            } else {
                                [origin_id].into()
                            };
                        }
                    }
                }
                if self.drag.as_ref().map_or(false, |d| d.active) {
                    if pos.x < SIDEBAR_WIDTH {
                        let n_folders = self.folders.len();
                        let albums_top =
                            SIDEBAR_ALBUMS_BASE_Y + n_folders as f32 * (ALBUM_ITEM_HEIGHT + 2.0);
                        let y_in_content = pos.y + self.sidebar_scroll_y - albums_top;
                        let row_h = ALBUM_ITEM_HEIGHT + 2.0;
                        self.drag_hover_album = if y_in_content >= 0.0 {
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
                        self.drag_hover_album = None;
                    }
                }
                Task::none()
            }

            Msg::MouseRightClicked => {
                let pos = self.cursor;
                if pos.x < SIDEBAR_WIDTH {
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
                            SidebarItem::AllFiles => None,
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
                        self.drag = Some(DragState {
                            origin_idx: idx,
                            start: pos,
                            cursor: pos,
                            active: false,
                        });
                    } else if pos.x > SIDEBAR_WIDTH {
                        let mods = self.modifiers;
                        if !mods.command() && !mods.shift() {
                            self.grid_selected.clear();
                            self.anchor_idx = None;
                        }
                    }
                }
                if self.detail.show && self.grid_selected.len() != 1 {
                    self.detail.file_id = None;
                    self.detail.tags.clear();
                    self.detail.rating = None;
                    self.detail.label = None;
                    self.detail.title = None;
                }
                Task::none()
            }

            Msg::MouseReleased => {
                let was_drag_active = self.drag.as_ref().map_or(false, |d| d.active);
                let drop_task = if was_drag_active {
                    self.drag_hover_album.clone().map(|id| {
                        let ids: Vec<String> = self.dragging_ids.iter().cloned().collect();
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

                self.drag = None;
                self.dragging_ids.clear();
                self.drag_hover_album = None;

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
                self.drag = None;
                self.dragging_ids.clear();
                self.drag_hover_album = None;
                let name = self
                    .albums
                    .iter()
                    .find(|a| a.id == album_id)
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                let count = ids.len();
                self.status = format!("Added {count} photo(s) to \"{name}\"");

                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        for fid in &ids {
                            let _ = db::add_file_to_album(&guard, &album_id, fid);
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
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                let wtx = self.watcher_tx.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let guard = conn.lock().unwrap();
                            scanner::scan_folder(&guard, &path, &|_| {}, &|prog| {
                                let _ = wtx.try_send(FileEvent::ScanProgress(prog));
                            })
                            .map(|r| r.total_count)
                            .unwrap_or(0)
                        })
                        .await
                        .unwrap_or(0)
                    },
                    Msg::ScanComplete,
                )
            }

            Msg::ScanComplete(count) => {
                self.is_scanning = false;
                self.status = format!("Scanned {count} photo(s)");
                let path = self.last_scanned_path.take();
                let t1 = self.load_sidebar_task();
                if let Some(p) = path {
                    Task::batch([t1, Task::done(Msg::SidebarItemClicked(SidebarItem::Folder(p)))])
                } else {
                    let t2 = self.load_files_task();
                    Task::batch([t1, t2])
                }
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
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        let _ = db::delete_files_by_root_folder(&guard, &path);
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
                let Some(conn) = self.conn.clone() else {
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
                        let guard = conn.lock().unwrap();
                        let _ = db::create_album(&guard, &album);
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
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        let _ = db::rename_album(&guard, &album_id, &name);
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
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        let _ = db::delete_album(&guard, &album_id);
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
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        for fid in &ids {
                            let _ = db::remove_file_from_album(&guard, &album_id, fid);
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
                let Some(conn) = self.conn.clone() else {
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
                        let guard = conn.lock().unwrap();
                        let _ = db::create_album(&guard, &album);
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
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        let _ = db::update_smart_album_query(&guard, &album_id, &query);
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
                rating,
                label,
                title,
            } => {
                self.detail.file_id = Some(file_id);
                self.detail.tags = tags;
                self.detail.rating = rating;
                self.detail.label = label;
                self.detail.title = title;
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
                self.detail.tags.push(tag);
                let Some(ref fid) = self.detail.file_id else {
                    return Task::none();
                };
                let fid = fid.clone();
                let tags = self.detail.tags.clone();
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap();
                        let _ = db::upsert_tags(&g, &fid, &tags);
                        db::get_all_tags(&g)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|(t, _)| t)
                            .collect::<Vec<_>>()
                    },
                    Msg::AllTagsLoaded,
                )
            }

            Msg::AddDetailTagDirect(tag) => {
                let tag = tag.trim().to_string();
                if tag.is_empty() || self.detail.tags.contains(&tag) {
                    return Task::none();
                }
                self.detail.tags.push(tag);
                self.detail.tag_input.clear();
                let Some(ref fid) = self.detail.file_id else {
                    return Task::none();
                };
                let fid = fid.clone();
                let tags = self.detail.tags.clone();
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap();
                        let _ = db::upsert_tags(&g, &fid, &tags);
                        db::get_all_tags(&g)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|(t, _)| t)
                            .collect::<Vec<_>>()
                    },
                    Msg::AllTagsLoaded,
                )
            }

            Msg::RemoveDetailTag(tag) => {
                self.detail.tags.retain(|t| t != &tag);
                let Some(ref fid) = self.detail.file_id else {
                    return Task::none();
                };
                let fid = fid.clone();
                let tags = self.detail.tags.clone();
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap();
                        let _ = db::upsert_tags(&g, &fid, &tags);
                        db::get_all_tags(&g)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|(t, _)| t)
                            .collect::<Vec<_>>()
                    },
                    Msg::AllTagsLoaded,
                )
            }

            Msg::AllTagsLoaded(tags) => {
                self.detail.all_tags = tags;
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
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap();
                        let _ = db::set_file_rating(&g, &fid, new_rating);
                    },
                    |()| Msg::NoOp,
                )
            }

            Msg::Reload => {
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::Tick => {
                while let Ok(ev) = self.thumbnail_rx.try_recv() {
                    match ev {
                        super::ThumbnailEvent::Ready(fid, path) => {
                            self.thumbnails
                                .insert(fid, isomfolio_core::models::ThumbnailState::Ready(path));
                            self.thumbnail_pending = self.thumbnail_pending.saturating_sub(1);
                        }
                        super::ThumbnailEvent::Failed(fid, _err) => {
                            self.thumbnails
                                .insert(fid, isomfolio_core::models::ThumbnailState::Failed(0));
                            self.thumbnail_pending = self.thumbnail_pending.saturating_sub(1);
                        }
                    }
                }
                if self.thumbnail_pending == 0
                    && self.thumbnail_total > 0
                    && self.thumbnail_done_at.is_none()
                {
                    self.thumbnail_done_at = Some(Instant::now());
                }
                if let Some(done_at) = self.thumbnail_done_at {
                    if done_at.elapsed() >= Duration::from_secs(2) {
                        self.thumbnail_total = 0;
                        self.thumbnail_done_at = None;
                    }
                }
                let mut tasks: Vec<Task<Msg>> = Vec::new();

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
                    if let Some(conn) = self.conn.clone() {
                        tasks.push(Task::perform(
                            async move {
                                let guard = conn.lock().unwrap();
                                for event in file_events {
                                    match event {
                                        FileEvent::Created(path) | FileEvent::Modified(path) => {
                                            let _ = scanner::resync_files(&guard, &[path]);
                                        }
                                        FileEvent::Deleted(path) => {
                                            let norm = normalize_path(&path);
                                            let fid = compute_file_id(&norm);
                                            let _ = db::mark_orphaned(&guard, &fid);
                                        }
                                        FileEvent::Renamed { old_path, new_path } => {
                                            let norm = normalize_path(&old_path);
                                            let old_fid = compute_file_id(&norm);
                                            let _ = db::mark_orphaned(&guard, &old_fid);
                                            let _ = scanner::resync_files(&guard, &[new_path]);
                                        }
                                        FileEvent::SidecarChanged(path) => {
                                            let _ = scanner::resync_sidecar_files(&guard, &[path]);
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
                if self.drag.as_ref().map_or(false, |d| d.active) {
                    self.drag_hover_album = opt_id;
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
                self.selected_recent_catalog = Some(path);
                Task::none()
            }

            Msg::OpenSelectedRecentCatalog => {
                let Some(path) = self.selected_recent_catalog.clone() else {
                    return Task::none();
                };
                Task::done(Msg::OpenCatalog(path))
            }

            Msg::ShowNewCatalogModal => {
                self.show_new_catalog_modal = true;
                self.new_catalog_dir = None;
                self.new_catalog_name.clear();
                Task::none()
            }

            Msg::HideNewCatalogModal => {
                self.show_new_catalog_modal = false;
                self.new_catalog_dir = None;
                self.new_catalog_name.clear();
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
                self.new_catalog_dir = Some(dir);
                Task::none()
            }

            Msg::NewCatalogNameChanged(s) => {
                self.new_catalog_name = s;
                Task::none()
            }

            Msg::ConfirmNewCatalog => {
                let Some(dir) = self.new_catalog_dir.clone() else {
                    return Task::none();
                };
                let name = self.new_catalog_name.trim().to_string();
                if name.is_empty() {
                    return Task::none();
                }
                self.show_new_catalog_modal = false;
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
                self.thumbnail_pool = None;
                self.thumbnail_pending = 0;
                self.thumbnail_total = 0;
                self.thumbnail_done_at = None;
                self.files.clear();
                self.thumbnails.clear();
                self.folders.clear();
                self.albums.clear();
                self.album_counts.clear();
                self.grid_selected.clear();
                self.drag = None;
                self.dragging_ids.clear();
                self.pending_search = None;
                self.search_text.clear();
                self.criteria = CriteriaState::default();
                self.detail = DetailState::default();
                self.selected_item = SidebarItem::AllFiles;
                self.scroll_y = 0.0;
                self.loupe_idx = 0;
                self.view_mode = ViewMode::Browse;
                self.album_pending_delete = None;
                self.folder_pending_remove = None;
                self.selected_recent_catalog = Some(path.clone());
                self.show_new_catalog_modal = false;
                self.new_catalog_dir = None;
                self.new_catalog_name.clear();
                isomfolio_core::app_paths::ensure_directories(&path);
                self.conn = db::open_database(&db_path(&path))
                    .ok()
                    .map(|c| std::sync::Arc::new(std::sync::Mutex::new(c)));
                self.status = if self.conn.is_none() {
                    "Error: could not open database — check permissions".to_string()
                } else {
                    String::new()
                };
                self.catalog_dir = path;
                self.show_welcome = false;
                self.tag_browser = None;
                self.recent_catalogs = isomfolio_core::app_paths::read_recent_catalogs();
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
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                let wtx = self.watcher_tx.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let guard = conn.lock().unwrap();
                            scanner::scan_folder(&guard, &path, &|_| {}, &|prog| {
                                let _ = wtx.try_send(FileEvent::ScanProgress(prog));
                            })
                            .map(|r| r.total_count)
                            .unwrap_or(0)
                        })
                        .await
                        .unwrap_or(0)
                    },
                    Msg::ScanComplete,
                )
            }

            Msg::DuplicateAlbum(album_id) => {
                self.context_menu = None;
                let Some(src) = self.albums.iter().find(|a| a.id == album_id).cloned() else {
                    return Task::none();
                };
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                let new_id = new_album_id();
                self.pending_album_select = Some(new_id.clone());
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        let new_album = Album {
                            id: new_id.clone(),
                            name: format!("{} copy", src.name),
                            kind: src.kind.clone(),
                            sort_order: 0,
                        };
                        let _ = db::create_album(&guard, &new_album);
                        if matches!(src.kind, AlbumKind::Manual) {
                            let _ = db::copy_album_files(&guard, &album_id, &new_id);
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
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        for fid in &ids {
                            let _ = db::add_file_to_album(&guard, &album_id, fid);
                        }
                    },
                    |()| Msg::DropCompleted,
                )
            }

            Msg::LoupeFullResLoaded { idx, handle } => {
                if self.loupe_idx == idx && matches!(self.view_mode, ViewMode::Loupe) {
                    self.loupe_full_res = Some((idx, handle));
                }
                Task::none()
            }

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
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap();
                        let _ = db::rename_prefixed_tags(&g, &old, &new_name);
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
                let Some(conn) = self.conn.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap();
                        let _ = db::delete_tag_with_descendants(&g, &tag);
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

            Msg::NoOp => Task::none(),
        }
    }
}

impl App {
    fn mark_smart_dirty(&mut self) {
        if self.current_album_is_smart() {
            self.smart_album_dirty = true;
        }
    }

    pub(crate) fn load_loupe_full_res(&self) -> Task<Msg> {
        let idx = self.loupe_idx;
        let Some(file) = self.files.get(idx) else {
            return Task::none();
        };
        let path = file.path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    iced::widget::image::Handle::from_path(&path)
                })
                .await
                .ok()
            },
            move |handle_opt| match handle_opt {
                Some(handle) => Msg::LoupeFullResLoaded { idx, handle },
                None => Msg::NoOp,
            },
        )
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
