use std::sync::atomic::{AtomicU64, Ordering};

use iced::Task;
use isomfolio_core::models::{Album, AlbumKind};

use super::LockUnwrap;
use super::super::{App, Msg, SidebarItem};

impl App {
    pub(super) fn handle_album_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
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
                        let failed = ids
                            .iter()
                            .filter(|fid| guard.add_file_to_album(&album_id, fid).is_err())
                            .count();
                        (count, failed)
                    },
                    |(total, failed)| {
                        if failed > 0 {
                            Msg::DbError(format!(
                                "{} added, {failed} failed to add to album",
                                total - failed
                            ))
                        } else {
                            Msg::DropCompleted
                        }
                    },
                )
            }

            Msg::DropCompleted => self.load_sidebar_task(),

            Msg::SetTargetAlbum(album_id) => {
                self.context_menu = None;
                if self.target_album.as_deref() == Some(album_id.as_str()) {
                    self.target_album = None;
                    self.status = "Cleared target album".to_string();
                } else {
                    let name = self
                        .albums
                        .iter()
                        .find(|a| a.id == album_id)
                        .map(|a| a.name.clone())
                        .unwrap_or_default();
                    self.target_album = Some(album_id);
                    self.status = format!("Target album: \"{name}\" — press B to add the selection");
                }
                Task::none()
            }

            Msg::AddSelectionToTargetAlbum => {
                let Some(target) = self.target_album.clone() else {
                    self.status =
                        "No target album — right-click an album → Set as Target Album".to_string();
                    return Task::none();
                };
                if self.grid_selected.is_empty() {
                    return Task::none();
                }
                Task::done(Msg::AddSelectionToAlbum(target))
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
                        let failed = ids
                            .iter()
                            .filter(|fid| guard.add_file_to_album(&album_id, fid).is_err())
                            .count();
                        (count, failed)
                    },
                    |(total, failed)| {
                        if failed > 0 {
                            Msg::DbError(format!(
                                "{} added, {failed} failed to add to album",
                                total - failed
                            ))
                        } else {
                            Msg::DropCompleted
                        }
                    },
                )
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
                let album = Album { id: new_album_id(), name, kind: AlbumKind::Manual, sort_order: 0 };
                self.pending_album_select = Some(album.id.clone());
                Task::perform(
                    async move {
                        let guard = conn.lock_unwrap();
                        guard.create_album(&album).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::AlbumCreated, Msg::DbError),
                )
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

            Msg::DeleteKeyPressed => {
                if self.grid_selected.is_empty() {
                    return Task::none();
                }
                match &self.selected_item {
                    // Inside a manual album, Delete unlinks from the album.
                    SidebarItem::Album(id)
                        if self
                            .albums
                            .iter()
                            .find(|a| &a.id == id)
                            .map(|a| matches!(a.kind, AlbumKind::Manual))
                            .unwrap_or(false) =>
                    {
                        self.handle_album_msg(Msg::ConfirmRemoveFromAlbum)
                    }
                    // Already in the Deleted view — nothing to delete further.
                    SidebarItem::Deleted => Task::none(),
                    // Everywhere else, Delete soft-deletes to the Deleted folder.
                    _ => Task::done(Msg::DeleteSelection),
                }
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
                // Open the filter panel so the inline name input is visible
                // (e.g. when triggered from the Photo menu with the panel shut).
                self.filters.show = true;
                self.filters.save_smart_input = Some(String::new());
                Task::none()
            }

            Msg::SmartAlbumNameChanged(s) => {
                self.filters.save_smart_input = Some(s);
                Task::none()
            }

            Msg::ConfirmSmartAlbum => {
                let name = self.filters.save_smart_input.take().unwrap_or_default();
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
                        guard
                            .update_smart_album_query(&album_id, &query)
                            .err()
                            .map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::SmartAlbumUpdated, Msg::DbError),
                )
            }

            _ => Task::none(),
        }
    }
}

pub(super) fn new_album_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("alb-{nanos:x}-{seq:x}")
}
