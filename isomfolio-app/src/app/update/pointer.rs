use std::collections::HashSet;

use iced::Task;

use super::super::{
    App, ContextMenuState, ContextMenuTarget, Drag, DragPayload, Msg, SidebarItem, ViewMode,
    SIDEBAR_HANDLE_WIDTH, SIDEBAR_WIDTH_MAX, SIDEBAR_WIDTH_MIN,
};

impl App {
    /// Pointer interaction: mouse press/move/release (drag gestures + grid-tile
    /// selection), sidebar/column resize, context-menu opens, and the sidebar
    /// album-row press. The drag *resolution* lives in `drag_drop.rs`; this is the
    /// raw-input half that drives it.
    pub(super) fn handle_pointer_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::SidebarResizeStart => {
                self.sidebar_resizing = true;
                Task::none()
            }

            Msg::ListColResizeStart(col) => {
                self.list_resize = Some(crate::app::ListResize {
                    col,
                    start_x: self.cursor.x,
                    start_w: self.list_col.get(col),
                });
                Task::none()
            }

            Msg::MouseMoved(pos) => {
                self.cursor = pos;
                if self.sidebar_resizing {
                    self.sidebar_width = pos.x.clamp(SIDEBAR_WIDTH_MIN, SIDEBAR_WIDTH_MAX);
                    return Task::none();
                }
                if let Some(r) = self.list_resize {
                    // Right-edge handle: width tracks the cursor's x delta.
                    self.list_col.set(r.col, r.start_w + (pos.x - r.start_x));
                    return Task::none();
                }
                if let Some(ref mut d) = self.drag.current {
                    d.cursor = pos;
                    if !d.past_threshold {
                        let dx = pos.x - d.start.x;
                        let dy = pos.y - d.start.y;
                        if (dx * dx + dy * dy).sqrt() > super::super::DRAG_THRESHOLD {
                            d.past_threshold = true;
                            // Snapshot the dragged photo set on activation: the whole
                            // multi-selection if the grabbed tile was part of it, else
                            // just that tile.
                            if let DragPayload::Photos { origin_idx, ids } = &mut d.payload {
                                let origin_id = self.files[*origin_idx].id.clone();
                                *ids = if self.grid_selected.contains(&origin_id) {
                                    self.grid_selected.clone()
                                } else {
                                    [origin_id].into()
                                };
                            }
                        }
                    }
                }
                Task::none()
            }

            Msg::MouseRightClicked => {
                let pos = self.cursor;
                if pos.x < self.sidebar_width + SIDEBAR_HANDLE_WIDTH {
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
                            SidebarItem::AllFiles
                            | SidebarItem::Deleted
                            | SidebarItem::Import(_) => None,
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
                    return self.handle_pointer_msg(Msg::MouseRightClicked);
                }
                self.context_menu = None;
                let pos = self.cursor;
                // A press in the List column-header strip (sort button or resize
                // handle) is handled by its own widget; don't clear the selection.
                if self.in_list_header_band(pos) {
                    return Task::none();
                }
                if matches!(self.view_mode, ViewMode::Browse) {
                    if let Some(idx) = self.tile_index_at(pos) {
                        let mods = self.modifiers;
                        let kind = if mods.command() {
                            ClickKind::Cmd
                        } else if mods.shift() {
                            ClickKind::Shift
                        } else {
                            ClickKind::Plain
                        };
                        let ids: Vec<&str> = self.files.iter().map(|f| f.id.as_str()).collect();
                        if let Some(out) = apply_grid_click(
                            &ids,
                            idx,
                            kind,
                            &self.grid_selected,
                            self.anchor_idx,
                            &self.selection_base,
                        ) {
                            self.grid_selected = out.selected;
                            self.anchor_idx = out.anchor;
                            self.select_lead = out.lead;
                            self.selection_base = out.base;
                        }
                        self.drag.current = Some(Drag {
                            payload: DragPayload::Photos {
                                origin_idx: idx,
                                ids: HashSet::new(),
                            },
                            start: pos,
                            cursor: pos,
                            past_threshold: false,
                        });
                    } else if pos.x > self.sidebar_width + SIDEBAR_HANDLE_WIDTH {
                        let mods = self.modifiers;
                        if !mods.command() && !mods.shift() {
                            self.grid_selected.clear();
                            self.selection_base.clear();
                            self.anchor_idx = None;
                            self.select_lead = None;
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
                if self.list_resize.take().is_some() {
                    return Task::none();
                }
                // Every drag/click resolves here: the press was captured either by a
                // tile (global `MousePressed`) or a sidebar row's `mouse_area`, and
                // this global release is where we know whether it travelled (a drop)
                // or stayed put (a click).
                let drag = self.drag.current.take();
                let hover = self.drag.hover.take();

                // A drag that didn't travel past the threshold is a plain click.
                // Album → navigate to it (and collapse the selection); group →
                // toggle its collapsed state (the header's default action); photo
                // falls through to the tile click/loupe handling.
                if let Some(Drag { payload, past_threshold: false, .. }) = &drag {
                    match payload {
                        DragPayload::Albums { pressed } => {
                            self.selected_albums.clear();
                            return Task::done(Msg::SidebarItemClicked(SidebarItem::Album(pressed.clone())));
                        }
                        DragPayload::Group { pressed } => {
                            return Task::done(Msg::ToggleGroupCollapsed(pressed.clone()));
                        }
                        DragPayload::Photos { .. } => {}
                    }
                }

                // A real album/group drag resolves entirely here (drop on a group,
                // or cancel and keep state if released off any target).
                if let Some(Drag {
                    payload: payload @ (DragPayload::Albums { .. } | DragPayload::Group { .. }),
                    past_threshold: true,
                    ..
                }) = &drag
                {
                    return self.resolve_drop(payload, hover);
                }

                // Photo payload (or a release with no tracked press): drop on an
                // album via the same matrix, else fall through to click/loupe.
                let was_drag_active = matches!(
                    &drag,
                    Some(Drag { payload: DragPayload::Photos { .. }, past_threshold: true, .. })
                );
                let drop_task = match &drag {
                    Some(Drag { payload, .. }) if was_drag_active => {
                        Some(self.resolve_drop(payload, hover))
                    }
                    _ => None,
                };

                let loupe_task: Option<Task<Msg>> =
                    if !was_drag_active && matches!(self.view_mode, ViewMode::Browse) {
                        if let Some(idx) = self.tile_index_at(self.cursor) {
                            // Plain click (no drag) on a tile that's part of a
                            // multi-selection collapses to just that tile. Deferred
                            // to release — a press-and-drag in between keeps the
                            // whole group so it can be dragged (Finder/Lightroom).
                            let mods = self.modifiers;
                            if !mods.command() && !mods.shift() && !mods.control() {
                                if let Some(id) = self.files.get(idx).map(|f| f.id.clone()) {
                                    if let Some(sel) =
                                        plain_release_collapse(&self.grid_selected, &id)
                                    {
                                        self.grid_selected = sel;
                                        self.selection_base.clear();
                                        self.anchor_idx = Some(idx);
                                        self.select_lead = Some(idx);
                                    }
                                }
                            }
                            if self
                                .last_click_time
                                .map_or(false, |t| t.elapsed().as_millis() < 300)
                            {
                                self.last_click_time = None;
                                Some(Task::done(Msg::OpenLoupe))
                            } else {
                                self.last_click_time = Some(std::time::Instant::now());
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

                // `drag`/`hover` were already taken above.
                let detail_task = self.maybe_load_detail();
                Task::batch([drop_task, loupe_task, Some(detail_task)].into_iter().flatten())
            }

            Msg::ModifiersChanged(m) => {
                self.modifiers = m;
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

            Msg::OpenAlbumsAddMenu => {
                // The "+" is a real button, so its left-press is captured and never
                // produces the global `MousePressed` that would clear the menu —
                // setting `context_menu` here is safe.
                self.context_menu = Some(ContextMenuState {
                    position: self.cursor,
                    target: ContextMenuTarget::AlbumsAdd,
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

            Msg::OpenSidebarEntityMenu(item) => {
                // Right-clicking the row is authoritative about which entity is
                // targeted — don't depend on a hover that may have been missed.
                self.hovered_sidebar_entity = Some(item);
                self.handle_pointer_msg(Msg::MouseRightClicked)
            }

            Msg::AlbumPressed(id) => {
                // Press-down on an album row (the row's `mouse_area` captured it).
                // Ctrl → menu, Cmd → toggle the multi-selection, plain → begin a
                // drag candidate whose click/drop is resolved on `MouseReleased`.
                self.context_menu = None;
                if self.modifiers.control() {
                    return self.handle_pointer_msg(Msg::OpenSidebarEntityMenu(SidebarItem::Album(id)));
                }
                if self.modifiers.command() {
                    if !self.selected_albums.remove(&id) {
                        self.selected_albums.insert(id);
                    }
                    return Task::none();
                }
                self.drag.current = Some(Drag {
                    payload: DragPayload::Albums { pressed: id },
                    start: self.cursor,
                    cursor: self.cursor,
                    past_threshold: false,
                });
                Task::none()
            }

            Msg::HoverDrop(target) => {
                // A droppable sidebar zone reporting enter (`Some`) or exit
                // (`None`). On exit we only clear if we're still pointing at the
                // zone that left, so a stale exit can't wipe a fresher enter.
                match target {
                    Some(t) => self.drag.hover = Some(t),
                    None => self.drag.hover = None,
                }
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

            _ => Task::none(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClickKind {
    Plain,
    Cmd,
    Shift,
}

pub(crate) struct SelOutcome {
    pub selected: HashSet<String>,
    pub anchor: Option<usize>,
    pub lead: Option<usize>,
    pub base: HashSet<String>,
}

/// File ids selected by a contiguous range `[anchor, lead]` unioned with the
/// `base` snapshot (disjoint tiles selected before the range began).
pub(crate) fn range_select(ids: &[&str], anchor: usize, lead: usize, base: &HashSet<String>) -> HashSet<String> {
    let mut sel = base.clone();
    let (lo, hi) = (anchor.min(lead), anchor.max(lead));
    for i in lo..=hi {
        if let Some(s) = ids.get(i) {
            sel.insert((*s).to_string());
        }
    }
    sel
}

/// Compute the new grid selection for a click on tile `idx`.
///
/// - **Plain** click on an unselected tile selects only it (fresh anchor, empty
///   base). Plain click on an already-selected tile returns `None` (no change at
///   press time — keeps the multi-selection so a press-and-drag can move the
///   whole group). If no drag follows, `MouseReleased` collapses the selection to
///   just the clicked tile.
/// - **Cmd** click toggles the tile and makes it the new pivot; the resulting
///   selection becomes the base, so a following Shift range preserves it.
/// - **Shift** click selects `base ∪ [anchor..=idx]`, replacing the previous
///   range — so clicking back toward the anchor *shrinks* the selection. The
///   anchor stays put; the clicked tile becomes the moving end (`lead`).
fn apply_grid_click(
    ids: &[&str],
    idx: usize,
    kind: ClickKind,
    selected: &HashSet<String>,
    anchor: Option<usize>,
    base: &HashSet<String>,
) -> Option<SelOutcome> {
    let id = ids.get(idx)?.to_string();
    match kind {
        ClickKind::Cmd => {
            let mut sel = selected.clone();
            if !sel.remove(&id) {
                sel.insert(id);
            }
            let base = sel.clone();
            Some(SelOutcome { selected: sel, anchor: Some(idx), lead: Some(idx), base })
        }
        ClickKind::Shift => {
            let a = anchor.unwrap_or(idx);
            let sel = range_select(ids, a, idx, base);
            Some(SelOutcome { selected: sel, anchor: Some(a), lead: Some(idx), base: base.clone() })
        }
        ClickKind::Plain => {
            if selected.contains(&id) {
                None
            } else {
                let mut sel = HashSet::new();
                sel.insert(id);
                Some(SelOutcome {
                    selected: sel,
                    anchor: Some(idx),
                    lead: Some(idx),
                    base: HashSet::new(),
                })
            }
        }
    }
}

/// On a plain-click release (no drag, no modifier) over a tile, collapse a
/// multi-selection down to just that tile. Returns the new single-tile selection,
/// or `None` to leave the selection unchanged — when the tile isn't selected, or
/// is already the sole selection (so a plain click that didn't change anything
/// doesn't churn state).
fn plain_release_collapse(selected: &HashSet<String>, id: &str) -> Option<HashSet<String>> {
    if selected.len() > 1 && selected.contains(id) {
        Some(std::iter::once(id.to_string()).collect())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod grid_selection {
        use super::*;

        const IDS: [&str; 6] = ["a", "b", "c", "d", "e", "f"];

        fn set(items: &[&str]) -> HashSet<String> {
            items.iter().map(|s| s.to_string()).collect()
        }

        fn click(
            idx: usize,
            kind: ClickKind,
            selected: &HashSet<String>,
            anchor: Option<usize>,
            base: &HashSet<String>,
        ) -> SelOutcome {
            apply_grid_click(&IDS, idx, kind, selected, anchor, base)
                .expect("expected a selection change")
        }

        #[test]
        fn plain_click_selects_only_that_tile() {
            let out = click(2, ClickKind::Plain, &set(&["a", "b"]), Some(0), &set(&["a", "b"]));
            assert_eq!(out.selected, set(&["c"]));
            assert_eq!(out.anchor, Some(2));
            assert_eq!(out.lead, Some(2));
            assert!(out.base.is_empty());
        }

        #[test]
        fn plain_click_on_selected_tile_keeps_selection() {
            // None == no change, so the multi-selection survives for a drag.
            let sel = set(&["a", "b", "c"]);
            assert!(apply_grid_click(&IDS, 1, ClickKind::Plain, &sel, Some(0), &sel).is_none());
        }

        #[test]
        fn shift_click_selects_inclusive_range_from_anchor() {
            // anchor a(0), shift-click d(3) → a..=d
            let out = click(3, ClickKind::Shift, &set(&["a"]), Some(0), &HashSet::new());
            assert_eq!(out.selected, set(&["a", "b", "c", "d"]));
            assert_eq!(out.anchor, Some(0));
            assert_eq!(out.lead, Some(3));
        }

        #[test]
        fn shift_click_back_toward_anchor_shrinks_the_range() {
            // The reported bug: a..=e selected, shift-click d → a..=d (e dropped).
            let after_first = click(4, ClickKind::Shift, &set(&["a"]), Some(0), &HashSet::new());
            assert_eq!(after_first.selected, set(&["a", "b", "c", "d", "e"]));

            let after_second = click(3, ClickKind::Shift, &after_first.selected, after_first.anchor, &after_first.base);
            assert_eq!(after_second.selected, set(&["a", "b", "c", "d"]));
            assert!(!after_second.selected.contains("e"));
        }

        #[test]
        fn shift_range_preserves_disjoint_cmd_selection_via_base() {
            // Cmd-click f to get a disjoint pick, then shift-range a..=c.
            let after_cmd = click(5, ClickKind::Cmd, &set(&["a"]), Some(0), &set(&["a"]));
            assert_eq!(after_cmd.selected, set(&["a", "f"]));
            assert_eq!(after_cmd.base, set(&["a", "f"])); // base captured for the next shift

            // anchor is f(5); shift-click c(2) → base ∪ [c..=f]
            let out = click(2, ClickKind::Shift, &after_cmd.selected, after_cmd.anchor, &after_cmd.base);
            assert_eq!(out.selected, set(&["a", "c", "d", "e", "f"]));
        }

        #[test]
        fn cmd_click_toggles_and_repivots() {
            let out = click(2, ClickKind::Cmd, &set(&["a", "b"]), Some(0), &set(&["a", "b"]));
            assert_eq!(out.selected, set(&["a", "b", "c"]));
            assert_eq!(out.anchor, Some(2));
            assert_eq!(out.base, set(&["a", "b", "c"]));

            // toggling c off again
            let off = click(2, ClickKind::Cmd, &out.selected, out.anchor, &out.base);
            assert_eq!(off.selected, set(&["a", "b"]));
        }

        #[test]
        fn shift_without_anchor_falls_back_to_clicked_tile() {
            let out = click(3, ClickKind::Shift, &HashSet::new(), None, &HashSet::new());
            assert_eq!(out.selected, set(&["d"]));
            assert_eq!(out.anchor, Some(3));
        }

        mod release_collapse {
            use super::*;

            #[test]
            fn collapses_multi_selection_to_the_clicked_tile() {
                let got = plain_release_collapse(&set(&["a", "b", "c"]), "b");
                assert_eq!(got, Some(set(&["b"])));
            }

            #[test]
            fn no_change_when_clicked_tile_is_already_the_sole_selection() {
                assert!(plain_release_collapse(&set(&["b"]), "b").is_none());
            }

            #[test]
            fn no_change_when_clicked_tile_is_not_selected() {
                // A plain click on an unselected tile is handled at press time;
                // release must not touch the selection.
                assert!(plain_release_collapse(&set(&["a", "b"]), "c").is_none());
            }

            #[test]
            fn no_change_on_empty_selection() {
                assert!(plain_release_collapse(&HashSet::new(), "a").is_none());
            }
        }
    }
}
