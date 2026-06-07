use std::sync::atomic::{AtomicU64, Ordering};

use iced::Task;
use isomfolio_core::models::Shelf;

use super::LockUnwrap;
use super::super::{App, ContextMenuState, ContextMenuTarget, Msg};

impl App {
    pub(super) fn handle_shelf_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::StartCreateShelf => {
                self.create_shelf_input = Some(String::new());
                iced::widget::operation::focus(crate::app::input_ids::create_shelf())
            }

            Msg::CreateShelfInputChanged(s) => {
                self.create_shelf_input = Some(s);
                Task::none()
            }

            Msg::ConfirmCreateShelf => {
                let name = self.create_shelf_input.take().unwrap_or_default();
                let name = name.trim().to_string();
                let pending = std::mem::take(&mut self.pending_shelf_albums);
                if name.is_empty() {
                    return Task::none();
                }
                if self.shelves.iter().any(|s| s.name.to_lowercase() == name.to_lowercase()) {
                    self.status = format!("A shelf named \u{201C}{name}\u{201D} already exists");
                    self.create_shelf_input = Some(name);
                    self.pending_shelf_albums = pending;
                    return Task::none();
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                let shelf = Shelf { id: new_shelf_id(), name, sort_order: 0 };
                if !pending.is_empty() {
                    self.selected_albums.clear();
                    self.status = format!(
                        "Filed {} album(s) under \u{201C}{}\u{201D}",
                        pending.len(),
                        shelf.name
                    );
                }
                Task::perform(
                    async move {
                        let guard = conn.lock_unwrap();
                        if let Err(e) = guard.create_shelf(&shelf) {
                            return Some(e.to_string());
                        }
                        pending
                            .iter()
                            .find_map(|aid| guard.set_album_shelf(aid, Some(&shelf.id)).err())
                            .map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::ShelfCreated, Msg::DbError),
                )
            }

            Msg::StartRenameShelf(shelf_id) => {
                self.context_menu = None;
                let current = self
                    .shelves
                    .iter()
                    .find(|s| s.id == shelf_id)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                self.rename_shelf_id = Some(shelf_id);
                self.rename_shelf_input = current;
                iced::widget::operation::focus(crate::app::input_ids::rename_shelf())
            }

            Msg::RenameShelfInputChanged(s) => {
                self.rename_shelf_input = s;
                Task::none()
            }

            Msg::ConfirmRenameShelf => {
                let name = self.rename_shelf_input.trim().to_string();
                let Some(shelf_id) = self.rename_shelf_id.take() else {
                    return Task::none();
                };
                if name.is_empty() {
                    return Task::none();
                }
                if self.shelves.iter().any(|s| s.id != shelf_id && s.name.to_lowercase() == name.to_lowercase()) {
                    self.status = format!("A shelf named \u{201C}{name}\u{201D} already exists");
                    self.rename_shelf_id = Some(shelf_id);
                    return Task::none();
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock_unwrap();
                        guard.rename_shelf(&shelf_id, &name).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::ShelfRenamed, Msg::DbError),
                )
            }

            Msg::RequestDeleteShelf(shelf_id) => {
                self.context_menu = None;
                self.shelf_pending_delete = Some(shelf_id);
                Task::none()
            }

            Msg::CancelDeleteShelf => {
                self.shelf_pending_delete = None;
                Task::none()
            }

            Msg::DeleteShelf(shelf_id) => {
                self.shelf_pending_delete = None;
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock_unwrap();
                        guard.delete_shelf(&shelf_id).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::ShelfDeleted, Msg::DbError),
                )
            }

            Msg::ToggleShelfCollapsed(shelf_id) => {
                if !self.collapsed_shelves.remove(&shelf_id) {
                    self.collapsed_shelves.insert(shelf_id);
                }
                Task::none()
            }

            Msg::ShelfHeaderPressed(shelf_id) => {
                if self.modifiers.control() {
                    return Task::done(Msg::OpenShelfMenu(shelf_id));
                }
                Task::done(Msg::ToggleShelfCollapsed(shelf_id))
            }

            Msg::OpenShelfMenu(shelf_id) => {
                self.context_menu = Some(ContextMenuState {
                    position: self.cursor,
                    target: ContextMenuTarget::Shelf(shelf_id),
                    submenu_open: false,
                });
                Task::none()
            }

            Msg::MoveAlbumsToShelf { album_ids, shelf_id } => {
                self.context_menu = None;
                self.selected_albums.clear();
                if album_ids.is_empty() {
                    return Task::none();
                }
                let count = album_ids.len();
                let dest = shelf_id
                    .as_deref()
                    .and_then(|sid| self.shelves.iter().find(|s| s.id == sid))
                    .map(|s| format!("\u{201C}{}\u{201D}", s.name))
                    .unwrap_or_else(|| "Ungrouped".to_string());
                self.status = format!("Moved {count} album(s) to {dest}");
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock_unwrap();
                        album_ids
                            .iter()
                            .find_map(|aid| {
                                guard.set_album_shelf(aid, shelf_id.as_deref()).err()
                            })
                            .map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::AlbumMovedToShelf, Msg::DbError),
                )
            }

            Msg::StartCreateShelfFor(album_ids) => {
                self.context_menu = None;
                self.pending_shelf_albums = album_ids;
                self.create_shelf_input = Some(String::new());
                iced::widget::operation::focus(crate::app::input_ids::create_shelf())
            }

            Msg::StartCreateAlbumIn(shelf_id) => {
                self.context_menu = None;
                // Reveal the shelf so the inline input appears inside it.
                self.collapsed_shelves.remove(&shelf_id);
                self.pending_album_shelf = Some(shelf_id);
                self.create_album_input = Some(String::new());
                iced::widget::operation::focus(crate::app::input_ids::create_album())
            }

            Msg::SelectShelfAlbums(shelf_id) => {
                self.context_menu = None;
                self.selected_albums = self
                    .albums
                    .iter()
                    .filter(|a| a.shelf_id.as_deref() == Some(shelf_id.as_str()))
                    .map(|a| a.id.clone())
                    .collect();
                Task::none()
            }

            Msg::ShelfCreated | Msg::ShelfRenamed | Msg::ShelfDeleted | Msg::AlbumMovedToShelf => {
                self.load_sidebar_task()
            }

            other => {
                debug_assert!(false, "handle_shelf_msg received misrouted message: {other:?}");
                Task::none()
            }
        }
    }
}

fn new_shelf_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("shf-{nanos:x}-{seq:x}")
}
