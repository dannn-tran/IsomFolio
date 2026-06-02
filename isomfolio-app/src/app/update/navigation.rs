use std::collections::HashSet;

use iced::Task;

use super::super::{
    App, ContextMenuState, ContextMenuTarget, DragState, LoupeState, Msg, SidebarItem, ViewMode,
    SIDEBAR_HANDLE_WIDTH, SIDEBAR_WIDTH_MAX, SIDEBAR_WIDTH_MIN, TILE_SIZE_MAX, TILE_SIZE_MIN,
    TILE_SIZE_STEP,
};

impl App {
    pub(super) fn handle_navigation_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::TileSizeUp => {
                self.tile_px = (self.tile_px + TILE_SIZE_STEP).min(TILE_SIZE_MAX);
                if let Some(idx) = self.anchor_idx { self.scroll_to_index(idx) } else { Task::none() }
            }

            Msg::TileSizeDown => {
                self.tile_px = (self.tile_px - TILE_SIZE_STEP).max(TILE_SIZE_MIN);
                if let Some(idx) = self.anchor_idx { self.scroll_to_index(idx) } else { Task::none() }
            }

            Msg::Navigate { dx, dy } => {
                if matches!(self.view_mode, ViewMode::Loupe | ViewMode::Preview) {
                    let total = self.files.len();
                    if total == 0 {
                        return Task::none();
                    }
                    let delta = dx + dy;
                    let new_idx =
                        (self.loupe.idx as i32 + delta).rem_euclid(total as i32) as usize;
                    self.loupe.idx = new_idx;
                    self.loupe.reset_zoom();
                    self.loupe.prefetch.retain(|&k, _| {
                        (k as i32 - new_idx as i32).unsigned_abs() as usize <= 2
                    });
                    if matches!(self.view_mode, ViewMode::Preview) {
                        self.anchor_idx = Some(new_idx);
                        self.grid_selected.clear();
                        if let Some(f) = self.files.get(new_idx) {
                            self.grid_selected.insert(f.id.clone());
                        }
                    }
                    let mut tasks: Vec<Task<Msg>> = Vec::new();
                    if let Some(handle) = self.loupe.prefetch.remove(&new_idx) {
                        // Already decoded by prefetch — show it directly. Do NOT
                        // also re-decode: a fresh decode mints a new texture id
                        // (`Handle::from_rgba` ids are unique-per-call), forcing
                        // the renderer to re-upload and flicker the same image.
                        self.loupe.full_res = Some((new_idx, handle));
                    } else {
                        self.loupe.full_res = None;
                        tasks.push(self.load_loupe_full_res());
                    }
                    tasks.push(self.load_loupe_prefetch());
                    if matches!(self.view_mode, ViewMode::Preview) {
                        tasks.push(self.scroll_to_index(new_idx));
                        tasks.push(self.maybe_load_detail());
                    }
                    return Task::batch(tasks);
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
                self.select_lead = Some(new_idx);
                self.grid_selected.clear();
                self.selection_base.clear();
                if let Some(f) = self.files.get(new_idx) {
                    self.grid_selected.insert(f.id.clone());
                }
                let scroll = self.scroll_to_index(new_idx);
                let detail = self.maybe_load_detail();
                Task::batch([scroll, detail])
            }

            Msg::NavigateExtend { dx, dy } => {
                if !matches!(self.view_mode, ViewMode::Browse) {
                    return self.handle_navigation_msg(Msg::Navigate { dx, dy });
                }
                let cols = self.cols().max(1) as i32;
                let total = self.files.len() as i32;
                if total == 0 {
                    return Task::none();
                }
                let anchor = self.anchor_idx.unwrap_or(0);
                self.anchor_idx = Some(anchor);
                let lead = self.select_lead.unwrap_or(anchor) as i32;
                let row = lead / cols;
                let col = lead % cols;
                let new_col = (col + dx).clamp(0, cols - 1);
                let new_row = (row + dy).clamp(0, (total - 1) / cols);
                let new_lead = (new_row * cols + new_col).min(total - 1) as usize;
                self.select_lead = Some(new_lead);
                let ids: Vec<&str> = self.files.iter().map(|f| f.id.as_str()).collect();
                self.grid_selected = range_select(&ids, anchor, new_lead, &self.selection_base);
                let scroll = self.scroll_to_index(new_lead);
                let detail = self.maybe_load_detail();
                Task::batch([scroll, detail])
            }

            Msg::LoupeZoomChanged { scale, pan } => {
                self.loupe.zoom = scale.clamp(
                    super::super::LOUPE_ZOOM_MIN,
                    super::super::LOUPE_ZOOM_MAX,
                );
                self.loupe.pan = if self.loupe.zoom <= super::super::LOUPE_ZOOM_MIN {
                    iced::Vector::ZERO
                } else {
                    pan
                };
                Task::none()
            }

            Msg::LoupeZoomBy(factor) => {
                let prev = self.loupe.zoom;
                let next = (prev * factor).clamp(
                    super::super::LOUPE_ZOOM_MIN,
                    super::super::LOUPE_ZOOM_MAX,
                );
                self.loupe.zoom = next;
                // Zoom toward the centre: scale the existing pan; the widget
                // clamps it to the image edges on the next draw.
                self.loupe.pan = if next <= super::super::LOUPE_ZOOM_MIN {
                    iced::Vector::ZERO
                } else {
                    self.loupe.pan * (next / prev)
                };
                Task::none()
            }

            Msg::LoupeZoomReset => {
                self.loupe.reset_zoom();
                Task::none()
            }

            Msg::OpenLoupe => {
                match self.view_mode {
                    ViewMode::Loupe => {
                        self.anchor_idx = Some(self.loupe.idx);
                        self.select_lead = Some(self.loupe.idx);
                        self.grid_selected.clear();
                        self.selection_base.clear();
                        if let Some(f) = self.files.get(self.loupe.idx) {
                            self.grid_selected.insert(f.id.clone());
                        }
                        self.view_mode = ViewMode::Browse;
                        self.loupe.full_res = None;
                        self.loupe.prefetch.clear();
                        return self.scroll_to_index(self.loupe.idx);
                    }
                    ViewMode::Preview => {
                        self.loupe.reset_zoom();
                        self.view_mode = ViewMode::Loupe;
                        return Task::none();
                    }
                    ViewMode::Browse => {
                        if !self.files.is_empty() {
                            let idx =
                                self.anchor_idx.unwrap_or(0).min(self.files.len() - 1);
                            self.loupe.idx = idx;
                            self.loupe.reset_zoom();
                            self.view_mode = ViewMode::Loupe;
                            if let Some(handle) = self.loupe.prefetch.remove(&idx) {
                                self.loupe.full_res = Some((idx, handle));
                                return self.load_loupe_prefetch();
                            }
                            self.loupe.full_res = None;
                            return Task::batch([
                                self.load_loupe_full_res(),
                                self.load_loupe_prefetch(),
                            ]);
                        }
                    }
                    ViewMode::People | ViewMode::Compare | ViewMode::Settings => {}
                }
                Task::none()
            }

            Msg::TogglePreview => {
                match self.view_mode {
                    ViewMode::Preview => {
                        self.view_mode = ViewMode::Browse;
                        self.loupe = LoupeState::default();
                    }
                    ViewMode::Browse => {
                        if let Some(idx) = self.anchor_idx {
                            self.loupe.idx = idx;
                            self.view_mode = ViewMode::Preview;
                            return Task::batch([
                                self.load_loupe_full_res(),
                                self.load_loupe_prefetch(),
                            ]);
                        }
                    }
                    _ => {}
                }
                Task::none()
            }

            Msg::SidebarResizeStart => {
                self.sidebar_resizing = true;
                Task::none()
            }

            Msg::MouseMoved(pos) => {
                self.cursor = pos;
                if self.sidebar_resizing {
                    self.sidebar_width = pos.x.clamp(SIDEBAR_WIDTH_MIN, SIDEBAR_WIDTH_MAX);
                    return Task::none();
                }
                if let Some(ref mut d) = self.drag.state {
                    d.cursor = pos;
                    if !d.active {
                        let dx = pos.x - d.start.x;
                        let dy = pos.y - d.start.y;
                        if (dx * dx + dy * dy).sqrt() > super::super::DRAG_THRESHOLD {
                            d.active = true;
                            let origin_idx = d.origin_idx;
                            let origin_id = self.files[origin_idx].id.clone();
                            self.drag.ids = if self.grid_selected.contains(&origin_id) {
                                self.grid_selected.clone()
                            } else {
                                [origin_id].into()
                            };
                        }
                    }
                }
                self.recompute_drag_hover();
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
                            SidebarItem::AllFiles | SidebarItem::FaceCluster(_) => None,
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
                    return self.handle_navigation_msg(Msg::MouseRightClicked);
                }
                self.context_menu = None;
                let pos = self.cursor;
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
                        self.drag.state = Some(DragState {
                            origin_idx: idx,
                            start: pos,
                            cursor: pos,
                            active: false,
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
                let was_drag_active = self.drag.state.as_ref().map_or(false, |d| d.active);
                let drop_task = if was_drag_active {
                    self.drag.hover_album.clone().map(|id| {
                        let ids: Vec<String> = self.drag.ids.iter().cloned().collect();
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

                self.drag.state = None;
                self.drag.ids.clear();
                self.drag.hover_album = None;

                let detail_task = self.maybe_load_detail();
                Task::batch([drop_task, loupe_task, Some(detail_task)].into_iter().flatten())
            }

            Msg::ModifiersChanged(m) => {
                self.modifiers = m;
                Task::none()
            }

            Msg::EscapePressed => {
                if self.open_menu.is_some() {
                    self.open_menu = None;
                    return Task::none();
                }
                if self.show_shortcut_help {
                    self.show_shortcut_help = false;
                    return Task::none();
                }
                if self.tag_browser.is_some() {
                    self.tag_browser = None;
                    return Task::none();
                }
                if self.context_menu.is_some() {
                    self.context_menu = None;
                    return Task::none();
                }
                if matches!(self.view_mode, ViewMode::Loupe) {
                    // Exit loupe back to the grid on the same photo (retain
                    // position) rather than jumping to the top.
                    self.anchor_idx = Some(self.loupe.idx);
                    self.select_lead = Some(self.loupe.idx);
                    self.grid_selected.clear();
                    self.selection_base.clear();
                    if let Some(f) = self.files.get(self.loupe.idx) {
                        self.grid_selected.insert(f.id.clone());
                    }
                    self.view_mode = ViewMode::Browse;
                    self.loupe.full_res = None;
                    self.loupe.prefetch.clear();
                    return self.scroll_to_index(self.loupe.idx);
                }
                if matches!(self.view_mode, ViewMode::Compare | ViewMode::Settings) {
                    self.view_mode = ViewMode::Browse;
                    return Task::none();
                }
                self.create_album_input = None;
                self.rename_album_id = None;
                self.faces.rename_cluster_id = None;
                self.filters.save_smart_input = None;
                self.remove_from_album_pending = false;
                self.reject_trash_pending = false;
                Task::none()
            }

            Msg::Scrolled { y, height, width } => {
                self.scroll_y = y;
                self.viewport_height = height;
                self.viewport_width = width;
                Task::none()
            }

            Msg::DragHoverAlbum(opt_id) => {
                if self.drag.state.as_ref().map_or(false, |d| d.active) {
                    self.drag.hover_album = opt_id;
                }
                Task::none()
            }

            Msg::OpenContextMenu(pos, target) => {
                self.context_menu =
                    Some(ContextMenuState { position: pos, target, submenu_open: false });
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
                self.recompute_drag_hover();
                Task::none()
            }

            Msg::HoverSidebarEntityEnd(item) => {
                if self.hovered_sidebar_entity.as_ref() == Some(&item) {
                    self.hovered_sidebar_entity = None;
                }
                self.recompute_drag_hover();
                Task::none()
            }

            Msg::ToggleShortcutHelp => {
                self.show_shortcut_help = !self.show_shortcut_help;
                Task::none()
            }

            Msg::OpenMenuDropdown(name) => {
                self.open_menu =
                    if self.open_menu.as_deref() == Some(name.as_str()) { None } else { Some(name) };
                Task::none()
            }

            Msg::HoverMenuTab(name) => {
                if self.open_menu.is_some() && self.open_menu.as_deref() != Some(name.as_str()) {
                    self.open_menu = Some(name);
                }
                Task::none()
            }

            Msg::CloseMenuDropdown => {
                self.open_menu = None;
                Task::none()
            }

            Msg::LoupeFullResLoaded { idx, handle } => {
                if self.loupe.idx == idx && matches!(self.view_mode, ViewMode::Loupe) {
                    self.loupe.full_res = Some((idx, handle));
                }
                Task::none()
            }

            Msg::LoupePrefetchLoaded { idx, handle } => {
                if matches!(self.view_mode, ViewMode::Loupe) {
                    let dist = (idx as i32 - self.loupe.idx as i32).unsigned_abs() as usize;
                    if dist <= 2 {
                        self.loupe.prefetch.insert(idx, handle);
                    }
                }
                Task::none()
            }

            Msg::SelectAll => {
                self.grid_selected = self.files.iter().map(|f| f.id.clone()).collect();
                self.selection_base = self.grid_selected.clone();
                if self.anchor_idx.is_none() && !self.files.is_empty() {
                    self.anchor_idx = Some(0);
                }
                Task::none()
            }

            Msg::DeselectAll => {
                self.grid_selected.clear();
                self.selection_base.clear();
                self.anchor_idx = None;
                self.select_lead = None;
                Task::none()
            }

            Msg::OpenCompare => {
                if self.grid_selected.len() != 2 {
                    self.status = "Select exactly 2 photos to compare".to_string();
                    return Task::none();
                }
                let mut sel = self.grid_selected.iter();
                let id0 = sel.next().expect("grid_selected.len() == 2 checked above").clone();
                let id1 = sel.next().expect("grid_selected.len() == 2 checked above").clone();
                let f0 = self.files.iter().find(|f| f.id == id0).cloned();
                let f1 = self.files.iter().find(|f| f.id == id1).cloned();
                self.compare = super::super::CompareState { files: [f0, f1], handles: [None, None] };
                self.view_mode = ViewMode::Compare;
                Task::batch([self.load_compare_slot(0), self.load_compare_slot(1)])
            }

            Msg::CompareFullResLoaded { slot, handle } => {
                if matches!(self.view_mode, ViewMode::Compare) {
                    self.compare.handles[slot] = Some(handle);
                }
                Task::none()
            }

            Msg::ShowInFinder(paths) => {
                self.context_menu = None;
                reveal_in_file_manager(&paths);
                Task::none()
            }

            Msg::SidebarScrolled(y) => {
                self.sidebar_scroll_y = y;
                Task::none()
            }

            _ => Task::none(),
        }
    }

    pub(crate) fn load_loupe_full_res(&self) -> Task<Msg> {
        let idx = self.loupe.idx;
        let Some(file) = self.files.get(idx) else { return Task::none() };
        let path = file.path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || decode_image_for_display(&path))
                    .await
                    .ok()
                    .flatten()
            },
            move |handle_opt| match handle_opt {
                Some(handle) => Msg::LoupeFullResLoaded { idx, handle },
                None => Msg::NoOp,
            },
        )
    }

    pub(crate) fn load_compare_slot(&self, slot: usize) -> Task<Msg> {
        let Some(file) = self.compare.files[slot].as_ref() else { return Task::none() };
        let path = file.path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || decode_image_for_display(&path))
                    .await
                    .ok()
                    .flatten()
            },
            move |handle_opt| match handle_opt {
                Some(handle) => Msg::CompareFullResLoaded { slot, handle },
                None => Msg::NoOp,
            },
        )
    }

    pub(crate) fn load_loupe_prefetch(&self) -> Task<Msg> {
        let total = self.files.len();
        if total == 0 {
            return Task::none();
        }
        let current = self.loupe.idx;
        let mut tasks = Vec::new();
        for delta in [-1i32, 1] {
            let idx = (current as i32 + delta).rem_euclid(total as i32) as usize;
            if self.loupe.prefetch.contains_key(&idx) {
                continue;
            }
            if self.loupe.full_res.as_ref().map_or(false, |(i, _)| *i == idx) {
                continue;
            }
            if let Some(file) = self.files.get(idx) {
                let path = file.path.clone();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || decode_image_for_display(&path))
                            .await
                            .ok()
                            .flatten()
                    },
                    move |handle_opt| match handle_opt {
                        Some(handle) => Msg::LoupePrefetchLoaded { idx, handle },
                        None => Msg::NoOp,
                    },
                ));
            }
        }
        Task::batch(tasks)
    }

    /// Resolve the album currently under the cursor as the drag drop-target,
    /// using the real rendered rows (`hovered_sidebar_entity`, kept up to date
    /// by each row's `mouse_area`) rather than hardcoded sidebar geometry — the
    /// folder tree's variable row count/height makes manual hit-testing wrong.
    pub(crate) fn recompute_drag_hover(&mut self) {
        if !self.drag.state.as_ref().map_or(false, |d| d.active) {
            return;
        }
        self.drag.hover_album =
            drop_album_for(self.hovered_sidebar_entity.as_ref(), &self.albums);
    }
}

/// The album a dragged selection would drop into, given the sidebar entity
/// under the cursor. Only **manual** albums are drop targets (smart albums are
/// criteria-defined; folders / People aren't album targets).
fn drop_album_for(
    hovered: Option<&SidebarItem>,
    albums: &[isomfolio_core::models::Album],
) -> Option<String> {
    match hovered {
        Some(SidebarItem::Album(id)) => albums
            .iter()
            .find(|a| &a.id == id)
            .filter(|a| matches!(a.kind, isomfolio_core::models::AlbumKind::Manual))
            .map(|a| a.id.clone()),
        _ => None,
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
fn range_select(ids: &[&str], anchor: usize, lead: usize, base: &HashSet<String>) -> HashSet<String> {
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
///   base). Plain click on an already-selected tile returns `None` (no change —
///   keeps the multi-selection so a drag can start).
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

fn decode_image_for_display(path: &str) -> Option<iced::widget::image::Handle> {
    let img = open_image(path)?;
    let rgba = img.into_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    Some(iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw()))
}

fn open_image(path: &str) -> Option<image::DynamicImage> {
    use isomfolio_core::indexing::thumbnail::is_raw_extension;
    use rawler::decoders::RawDecodeParams;
    use rawler::rawsource::RawSource;
    use std::path::Path;

    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if is_raw_extension(ext) {
        let source = RawSource::new(Path::new(path)).ok()?;
        let decoder = rawler::get_decoder(&source).ok()?;
        let params = RawDecodeParams::default();
        return decoder.full_image(&source, &params).ok().flatten()
            .or_else(|| decoder.preview_image(&source, &params).ok().flatten());
    }

    image::open(path).ok()
}

#[cfg(target_os = "macos")]
fn reveal_in_file_manager(paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    if paths.len() == 1 {
        let _ = std::process::Command::new("open").arg("-R").arg(&paths[0]).spawn();
    } else {
        let file_list = paths
            .iter()
            .map(|p| format!("POSIX file \"{}\"", p.replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(", ");
        let script = format!(
            "tell application \"Finder\"\nreveal {{{file_list}}}\nactivate\nend tell"
        );
        let _ = std::process::Command::new("osascript").arg("-e").arg(&script).spawn();
    }
}

#[cfg(target_os = "windows")]
fn reveal_in_file_manager(paths: &[String]) {
    // explorer /select only accepts one path; open one window per unique parent folder
    let mut seen = std::collections::HashSet::new();
    for path in paths {
        let folder = std::path::Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if seen.insert(folder) {
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,{path}"))
                .spawn();
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn reveal_in_file_manager(paths: &[String]) {
    // Open each unique parent folder via xdg-open
    let mut seen = std::collections::HashSet::new();
    for path in paths {
        let folder = std::path::Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if seen.insert(folder.clone()) {
            let _ = std::process::Command::new("xdg-open").arg(&folder).spawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use isomfolio_core::models::{Album, AlbumKind, SearchQuery};

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
    }

    mod drop_album_for {
        use super::*;

        fn manual(id: &str) -> Album {
            Album { id: id.into(), name: id.into(), kind: AlbumKind::Manual, sort_order: 0 }
        }
        fn smart(id: &str) -> Album {
            Album {
                id: id.into(),
                name: id.into(),
                kind: AlbumKind::Smart(SearchQuery::default()),
                sort_order: 0,
            }
        }

        #[test]
        fn manual_album_under_cursor_is_the_drop_target() {
            let albums = vec![manual("a"), manual("b")];
            let hovered = SidebarItem::Album("b".into());
            assert_eq!(drop_album_for(Some(&hovered), &albums), Some("b".into()));
        }

        #[test]
        fn smart_album_is_not_a_drop_target() {
            let albums = vec![smart("s")];
            let hovered = SidebarItem::Album("s".into());
            assert_eq!(drop_album_for(Some(&hovered), &albums), None);
        }

        #[test]
        fn folder_or_nothing_under_cursor_yields_no_target() {
            let albums = vec![manual("a")];
            assert_eq!(drop_album_for(Some(&SidebarItem::Folder("/x".into())), &albums), None);
            assert_eq!(drop_album_for(None, &albums), None);
        }

        #[test]
        fn unknown_album_id_yields_no_target() {
            let albums = vec![manual("a")];
            let hovered = SidebarItem::Album("ghost".into());
            assert_eq!(drop_album_for(Some(&hovered), &albums), None);
        }
    }
}
