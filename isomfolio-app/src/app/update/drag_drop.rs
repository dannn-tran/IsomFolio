use std::collections::HashSet;

use iced::Task;

use super::super::{App, DragPayload, DropTarget, Msg};
use isomfolio_core::models::AlbumId;

impl App {
    /// Turn a finished drag into its drop action, validated through the
    /// `drop_allowed` matrix. Returns `Task::none()` when released off any
    /// compatible target. The single dispatch point for every payload.
    pub(super) fn resolve_drop(&self, payload: &DragPayload, target: Option<DropTarget>) -> Task<Msg> {
        let Some(target) = target else { return Task::none() };
        if !drop_allowed(payload, &target) {
            return Task::none();
        }
        match (payload, target) {
            (DragPayload::Photos { ids, .. }, DropTarget::Album(album_id)) => {
                Task::done(Msg::DroppedToAlbum(album_id, ids.iter().cloned().collect()))
            }
            (DragPayload::Albums { pressed }, DropTarget::Group(group_id)) => {
                let album_ids = dragged_albums(pressed, &self.selected_albums);
                Task::done(Msg::MoveAlbumsToGroup { album_ids, group_id: Some(group_id) })
            }
            (DragPayload::Group { pressed }, DropTarget::Group(group_id)) => {
                // Dropping a group on itself is a no-op; deeper cycles are caught
                // by the catalog's descendant check, which leaves the tree intact.
                if pressed == &group_id {
                    return Task::none();
                }
                Task::done(Msg::MoveGroupToParent {
                    group_id: pressed.clone(),
                    parent_id: Some(group_id),
                })
            }
            _ => Task::none(),
        }
    }
}

/// Every album sharing a group (or the ungrouped top level) with something
/// already selected — what `Cmd+A` expands an album selection to, like Cmd+A
/// within a Finder folder. The set of "containers" is derived from the current
/// selection, then every album in those containers is selected.
pub(crate) fn album_siblings(
    albums: &[isomfolio_core::models::Album],
    selected: &HashSet<AlbumId>,
) -> HashSet<AlbumId> {
    let groups: HashSet<Option<String>> = albums
        .iter()
        .filter(|a| selected.contains(&a.id))
        .map(|a| a.group_id.clone())
        .collect();
    albums
        .iter()
        .filter(|a| groups.contains(&a.group_id))
        .map(|a| a.id.clone())
        .collect()
}

/// The albums a group drop applies to: the whole multi-selection when the
/// pressed album is part of it, otherwise just the pressed album. Mirrors the
/// grid's "drag the group if you grabbed a selected tile" rule.
pub(crate) fn dragged_albums(pressed: &str, selected: &HashSet<AlbumId>) -> Vec<AlbumId> {
    if selected.len() > 1 && selected.contains(pressed) {
        selected.iter().cloned().collect()
    } else {
        vec![pressed.to_string()]
    }
}

/// The drop-compatibility matrix: which drag payloads may be released onto which
/// targets. The single source of truth shared by the release handler and the
/// sidebar's drop-zone mounting: photos → albums, albums → groups, groups →
/// groups (nesting).
pub(crate) fn drop_allowed(payload: &DragPayload, target: &DropTarget) -> bool {
    matches!(
        (payload, target),
        (DragPayload::Photos { .. }, DropTarget::Album(_))
            | (DragPayload::Albums { .. }, DropTarget::Group(_))
            | (DragPayload::Group { .. }, DropTarget::Group(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use isomfolio_core::models::Album;

    mod dragged_albums_fn {
        use super::*;

        fn set(items: &[&str]) -> HashSet<AlbumId> {
            items.iter().map(|s| s.to_string()).collect()
        }

        #[test]
        fn unselected_album_drags_only_itself() {
            let got = dragged_albums("a", &set(&["b", "c"]));
            assert_eq!(got, vec!["a".to_string()]);
        }

        #[test]
        fn lone_selection_drags_only_the_pressed_album() {
            // A single-album "selection" is just that album, not a group.
            let got = dragged_albums("a", &set(&["a"]));
            assert_eq!(got, vec!["a".to_string()]);
        }

        #[test]
        fn pressing_a_member_drags_the_whole_selection() {
            let mut got = dragged_albums("a", &set(&["a", "b", "c"]));
            got.sort();
            assert_eq!(got, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        }

        #[test]
        fn pressing_a_non_member_drags_only_it_even_with_a_selection() {
            let got = dragged_albums("z", &set(&["a", "b"]));
            assert_eq!(got, vec!["z".to_string()]);
        }
    }

    mod album_siblings_fn {
        use super::*;
        use isomfolio_core::models::AlbumKind;

        fn album(id: &str, group: Option<&str>) -> Album {
            Album {
                id: id.to_string(),
                name: id.to_string(),
                kind: AlbumKind::Manual,
                sort_order: 0,
                group_id: group.map(|s| s.to_string()),
            }
        }

        fn set(items: &[&str]) -> HashSet<AlbumId> {
            items.iter().map(|s| s.to_string()).collect()
        }

        #[test]
        fn selecting_one_album_expands_to_its_whole_group() {
            let albums = vec![
                album("a", Some("s1")),
                album("b", Some("s1")),
                album("c", Some("s2")),
                album("d", None),
            ];
            let mut got: Vec<_> = album_siblings(&albums, &set(&["a"])).into_iter().collect();
            got.sort();
            assert_eq!(got, vec!["a".to_string(), "b".to_string()]);
        }

        #[test]
        fn ungrouped_selection_expands_to_all_ungrouped_albums() {
            let albums = vec![
                album("a", Some("s1")),
                album("d", None),
                album("e", None),
            ];
            let mut got: Vec<_> = album_siblings(&albums, &set(&["d"])).into_iter().collect();
            got.sort();
            assert_eq!(got, vec!["d".to_string(), "e".to_string()]);
        }

        #[test]
        fn selection_spanning_two_groups_expands_to_both() {
            let albums = vec![
                album("a", Some("s1")),
                album("b", Some("s1")),
                album("c", Some("s2")),
                album("d", Some("s3")),
            ];
            let mut got: Vec<_> =
                album_siblings(&albums, &set(&["a", "c"])).into_iter().collect();
            got.sort();
            assert_eq!(got, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        }
    }

    mod drop_compat {
        use super::*;

        fn photos() -> DragPayload {
            DragPayload::Photos { origin_idx: 0, ids: HashSet::new() }
        }
        fn albums() -> DragPayload {
            DragPayload::Albums { pressed: "a".into() }
        }
        fn group() -> DragPayload {
            DragPayload::Group { pressed: "g".into() }
        }

        #[test]
        fn photos_drop_onto_albums_only() {
            assert!(drop_allowed(&photos(), &DropTarget::Album("a".into())));
            assert!(!drop_allowed(&photos(), &DropTarget::Group("s".into())));
        }

        #[test]
        fn albums_drop_onto_groups_only() {
            assert!(drop_allowed(&albums(), &DropTarget::Group("s".into())));
            assert!(!drop_allowed(&albums(), &DropTarget::Album("a".into())));
        }

        #[test]
        fn groups_drop_onto_groups_only() {
            assert!(drop_allowed(&group(), &DropTarget::Group("s".into())));
            assert!(!drop_allowed(&group(), &DropTarget::Album("a".into())));
        }
    }
}
