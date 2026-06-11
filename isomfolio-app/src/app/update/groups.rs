use std::sync::atomic::{AtomicU64, Ordering};

use iced::Task;
use isomfolio_core::models::Group;

use super::LockUnwrap;
use super::super::{App, ContextMenuState, ContextMenuTarget, Drag, DragPayload, Msg};

impl App {
    pub(super) fn handle_group_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::StartCreateGroup => {
                self.create_group_input = Some(String::new());
                iced::widget::operation::focus(crate::app::input_ids::create_group())
            }

            Msg::CreateGroupInputChanged(s) => {
                self.create_group_input = Some(s);
                Task::none()
            }

            Msg::ConfirmCreateGroup => {
                let name = self.create_group_input.take().unwrap_or_default();
                let name = name.trim().to_string();
                let pending = std::mem::take(&mut self.pending_group_albums);
                let parent_id = self.pending_group_parent.take();
                if name.is_empty() {
                    return Task::none();
                }
                if self.groups.iter().any(|s| s.name.to_lowercase() == name.to_lowercase()) {
                    self.status = format!("A group named \u{201C}{name}\u{201D} already exists");
                    self.create_group_input = Some(name);
                    self.pending_group_albums = pending;
                    self.pending_group_parent = parent_id;
                    return Task::none();
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                let group = Group { id: new_group_id(), name, sort_order: 0, parent_id };
                if !pending.is_empty() {
                    self.selected_albums.clear();
                    self.status = format!(
                        "Filed {} album(s) under \u{201C}{}\u{201D}",
                        pending.len(),
                        group.name
                    );
                }
                Task::perform(
                    async move {
                        let guard = conn.lock_unwrap();
                        if let Err(e) = guard.create_group(&group) {
                            return Some(e.to_string());
                        }
                        pending
                            .iter()
                            .find_map(|aid| guard.set_album_group(aid, Some(&group.id)).err())
                            .map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::GroupCreated, Msg::DbError),
                )
            }

            Msg::StartRenameGroup(group_id) => {
                self.context_menu = None;
                let current = self
                    .groups
                    .iter()
                    .find(|s| s.id == group_id)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                self.rename_group_id = Some(group_id);
                self.rename_group_input = current;
                iced::widget::operation::focus(crate::app::input_ids::rename_group())
            }

            Msg::RenameGroupInputChanged(s) => {
                self.rename_group_input = s;
                Task::none()
            }

            Msg::ConfirmRenameGroup => {
                let name = self.rename_group_input.trim().to_string();
                let Some(group_id) = self.rename_group_id.take() else {
                    return Task::none();
                };
                if name.is_empty() {
                    return Task::none();
                }
                if self.groups.iter().any(|s| s.id != group_id && s.name.to_lowercase() == name.to_lowercase()) {
                    self.status = format!("A group named \u{201C}{name}\u{201D} already exists");
                    self.rename_group_id = Some(group_id);
                    return Task::none();
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock_unwrap();
                        guard.rename_group(&group_id, &name).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::GroupRenamed, Msg::DbError),
                )
            }

            Msg::RequestDeleteGroup(group_id) => {
                self.context_menu = None;
                self.group_pending_delete = Some(group_id);
                Task::none()
            }

            Msg::CancelDeleteGroup => {
                self.group_pending_delete = None;
                Task::none()
            }

            Msg::DeleteGroup(group_id) => {
                self.group_pending_delete = None;
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock_unwrap();
                        guard.delete_group(&group_id).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::GroupDeleted, Msg::DbError),
                )
            }

            Msg::ToggleGroupCollapsed(group_id) => {
                if !self.collapsed_groups.remove(&group_id) {
                    self.collapsed_groups.insert(group_id);
                }
                Task::none()
            }

            Msg::GroupHeaderPressed(group_id) => {
                self.context_menu = None;
                if self.modifiers.control() {
                    return Task::done(Msg::OpenGroupMenu(group_id));
                }
                // Begin a drag candidate. A plain press (no travel) toggles the
                // group collapsed on release; a real drag nests it under another
                // group — both resolved in `MouseReleased`, mirroring albums.
                self.drag.current = Some(Drag {
                    payload: DragPayload::Group { pressed: group_id },
                    start: self.cursor,
                    cursor: self.cursor,
                    past_threshold: false,
                });
                Task::none()
            }

            Msg::OpenGroupMenu(group_id) => {
                self.context_menu = Some(ContextMenuState {
                    position: self.cursor,
                    target: ContextMenuTarget::Group(group_id),
                    submenu_open: false,
                });
                Task::none()
            }

            Msg::MoveAlbumsToGroup { album_ids, group_id } => {
                self.context_menu = None;
                self.selected_albums.clear();
                if album_ids.is_empty() {
                    return Task::none();
                }
                let count = album_ids.len();
                let dest = group_id
                    .as_deref()
                    .and_then(|sid| self.groups.iter().find(|s| s.id == sid))
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
                                guard.set_album_group(aid, group_id.as_deref()).err()
                            })
                            .map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::AlbumMovedToGroup, Msg::DbError),
                )
            }

            Msg::StartCreateGroupFor(album_ids) => {
                self.context_menu = None;
                self.pending_group_albums = album_ids;
                self.create_group_input = Some(String::new());
                iced::widget::operation::focus(crate::app::input_ids::create_group())
            }

            Msg::StartCreateGroupIn(parent_id) => {
                self.context_menu = None;
                // Reveal the parent so the inline input appears nested inside it.
                self.collapsed_groups.remove(&parent_id);
                self.pending_group_parent = Some(parent_id);
                self.create_group_input = Some(String::new());
                iced::widget::operation::focus(crate::app::input_ids::create_group())
            }

            Msg::StartCreateAlbumIn(group_id) => {
                self.context_menu = None;
                // Reveal the group so the inline input appears inside it.
                self.collapsed_groups.remove(&group_id);
                self.pending_album_group = Some(group_id);
                self.create_album_input = Some(String::new());
                iced::widget::operation::focus(crate::app::input_ids::create_album())
            }

            Msg::SelectGroupAlbums(group_id) => {
                self.context_menu = None;
                self.selected_albums = self
                    .albums
                    .iter()
                    .filter(|a| a.group_id.as_deref() == Some(group_id.as_str()))
                    .map(|a| a.id.clone())
                    .collect();
                Task::none()
            }

            Msg::MoveGroupToParent { group_id, parent_id } => {
                self.context_menu = None;
                let name = self
                    .groups
                    .iter()
                    .find(|g| g.id == group_id)
                    .map(|g| g.name.clone())
                    .unwrap_or_default();
                // Reject cycles up front from the in-memory tree (the catalog
                // guards too, as defense-in-depth) so the status reads as user
                // feedback, not a DB error, and the tree is left untouched.
                if move_would_cycle(&self.groups, &group_id, parent_id.as_deref()) {
                    self.status =
                        format!("Can't nest \u{201C}{name}\u{201D} there — a group can't go inside itself");
                    return Task::none();
                }
                let dest = parent_id
                    .as_deref()
                    .and_then(|pid| self.groups.iter().find(|g| g.id == pid))
                    .map(|g| format!("inside \u{201C}{}\u{201D}", g.name))
                    .unwrap_or_else(|| "to the top level".to_string());
                self.status = format!("Moved \u{201C}{name}\u{201D} {dest}");
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let guard = conn.lock_unwrap();
                        guard
                            .set_group_parent(&group_id, parent_id.as_deref())
                            .err()
                            .map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::GroupMoved, Msg::DbError),
                )
            }

            Msg::GroupCreated | Msg::GroupRenamed | Msg::GroupDeleted | Msg::GroupMoved | Msg::AlbumMovedToGroup => {
                self.load_sidebar_task()
            }

            other => {
                debug_assert!(false, "handle_group_msg received misrouted message: {other:?}");
                Task::none()
            }
        }
    }

}

/// True if filing `group_id` under `parent_id` would form a cycle — i.e. `parent`
/// is the group itself or sits within its subtree. Walks up the in-memory parent
/// chain from `parent`; hitting `group_id` means the group is an ancestor of the
/// proposed parent, so the move must be rejected. Mirrors the catalog's
/// `group_is_descendant` CTE as the user-facing pre-check. A bounded walk: the
/// chain length is capped by the group count, so even a malformed (cyclic) tree
/// terminates instead of looping forever.
fn move_would_cycle(groups: &[Group], group_id: &str, parent_id: Option<&str>) -> bool {
    let mut cur = parent_id;
    let mut steps = 0;
    while let Some(c) = cur {
        if c == group_id {
            return true;
        }
        steps += 1;
        if steps > groups.len() {
            // More hops than groups means the data already contains a cycle that
            // doesn't pass through `group_id`; stop rather than spin.
            return true;
        }
        cur = groups.iter().find(|g| g.id == c).and_then(|g| g.parent_id.as_deref());
    }
    false
}

fn new_group_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("grp-{nanos:x}-{seq:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    mod move_would_cycle_fn {
        use super::*;

        fn g(id: &str, parent: Option<&str>) -> Group {
            Group { id: id.into(), name: id.into(), sort_order: 0, parent_id: parent.map(Into::into) }
        }

        // Tree: a → b → c (c child of b, b child of a), plus sibling d.
        fn tree() -> Vec<Group> {
            vec![g("a", None), g("b", Some("a")), g("c", Some("b")), g("d", None)]
        }

        #[test]
        fn nesting_a_group_under_itself_is_a_cycle() {
            assert!(move_would_cycle(&tree(), "a", Some("a")));
        }

        #[test]
        fn nesting_a_group_under_its_direct_child_is_a_cycle() {
            assert!(move_would_cycle(&tree(), "a", Some("b")));
        }

        #[test]
        fn nesting_a_group_under_a_deeper_descendant_is_a_cycle() {
            assert!(move_would_cycle(&tree(), "a", Some("c")));
        }

        #[test]
        fn nesting_under_a_non_descendant_is_allowed() {
            assert!(!move_would_cycle(&tree(), "a", Some("d")));
            // Filing a leaf under another branch is fine.
            assert!(!move_would_cycle(&tree(), "c", Some("d")));
        }

        #[test]
        fn moving_to_the_top_level_is_never_a_cycle() {
            assert!(!move_would_cycle(&tree(), "c", None));
        }

        #[test]
        fn malformed_cyclic_data_terminates_instead_of_looping() {
            // x ↔ y point at each other; the move doesn't pass through `c`, but the
            // walk must still stop (bounded by group count).
            let groups = vec![g("x", Some("y")), g("y", Some("x")), g("c", None)];
            assert!(move_would_cycle(&groups, "c", Some("x")));
        }
    }
}
