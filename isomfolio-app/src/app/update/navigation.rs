use std::collections::HashSet;

use iced::{Point, Task};

use super::super::{
    loupe, App, ContextMenuState, ContextMenuTarget, Drag, DragPayload, DropTarget, LoupeLoadError,
    LoupeState, Msg, SidebarItem, ViewMode, SIDEBAR_HANDLE_WIDTH, SIDEBAR_WIDTH_MAX,
    SIDEBAR_WIDTH_MIN, TILE_SIZE_MAX, TILE_SIZE_MIN, TILE_SIZE_STEP,
};
use isomfolio_core::models::AlbumId;

impl App {
    pub(super) fn handle_navigation_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::TileSizeUp => {
                // In the loupe the "zoom" keys zoom the image, not the grid tiles.
                if matches!(self.view_mode, ViewMode::Loupe) {
                    return self.handle_navigation_msg(Msg::LoupeZoomBy(1.25));
                }
                self.tile_px = (self.tile_px + TILE_SIZE_STEP).min(TILE_SIZE_MAX);
                if let Some(idx) = self.anchor_idx { self.scroll_to_index(idx) } else { Task::none() }
            }

            Msg::TileSizeDown => {
                if matches!(self.view_mode, ViewMode::Loupe) {
                    return self.handle_navigation_msg(Msg::LoupeZoomBy(0.8));
                }
                self.tile_px = (self.tile_px - TILE_SIZE_STEP).max(TILE_SIZE_MIN);
                if let Some(idx) = self.anchor_idx { self.scroll_to_index(idx) } else { Task::none() }
            }

            Msg::SetTileSize(px) => {
                self.tile_px = px.clamp(TILE_SIZE_MIN, TILE_SIZE_MAX);
                if let Some(idx) = self.anchor_idx { self.scroll_to_index(idx) } else { Task::none() }
            }

            Msg::WindowResized(width) => {
                self.window_width = width;
                Task::none()
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
                    self.loupe.load_error = None;
                    if self.loupe.lock_zoom {
                        // Keep zoom+pan; the new photo still needs its own hi-res
                        // decode if we're zoomed in.
                        self.loupe.hires_loaded = false;
                    } else {
                        self.loupe.reset_zoom();
                    }
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
                    // Held zoom into a new photo: load its full-res so the kept
                    // zoom level is pixel-accurate, not a blown-up preview.
                    if self.loupe.lock_zoom && self.loupe.zoom > super::super::LOUPE_ZOOM_MIN {
                        tasks.push(self.load_loupe_hires());
                    }
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
                let new_idx = grid_step(current, dx, dy, cols, total) as usize;
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
                let new_lead = grid_step(lead, dx, dy, cols, total) as usize;
                self.select_lead = Some(new_lead);
                let ids: Vec<&str> = self.files.iter().map(|f| f.id.as_str()).collect();
                self.grid_selected = range_select(&ids, anchor, new_lead, &self.selection_base);
                let scroll = self.scroll_to_index(new_lead);
                let detail = self.maybe_load_detail();
                Task::batch([scroll, detail])
            }

            // The widget already reduced this through `LoupeGeometry::apply`
            // (it owns the live geometry); the app just stores the result and
            // loads the hi-res decode if we ended up zoomed in.
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
                if self.loupe.zoom > super::super::LOUPE_ZOOM_MIN {
                    return self.load_loupe_hires();
                }
                Task::none()
            }

            Msg::LoupeZoomBy(factor) => {
                let anchor = self.loupe_center();
                self.apply_loupe_intent(loupe::LoupeIntent::ZoomAround { anchor, factor })
            }

            Msg::LoupeZoomReset => self.apply_loupe_intent(loupe::LoupeIntent::Reset),

            Msg::ToggleLoupeZoomLock => {
                self.loupe.lock_zoom = !self.loupe.lock_zoom;
                Task::none()
            }

            Msg::LoupeJumpTo(idx) => {
                if idx >= self.files.len() || !matches!(self.view_mode, ViewMode::Loupe) {
                    return Task::none();
                }
                self.loupe.idx = idx;
                self.loupe.reset_zoom();
                self.loupe.load_error = None;
                self.loupe.prefetch.retain(|&k, _| {
                    (k as i32 - idx as i32).unsigned_abs() as usize <= 2
                });
                let mut tasks: Vec<Task<Msg>> = Vec::new();
                if let Some(handle) = self.loupe.prefetch.remove(&idx) {
                    self.loupe.full_res = Some((idx, handle));
                } else {
                    self.loupe.full_res = None;
                    tasks.push(self.load_loupe_full_res());
                }
                tasks.push(self.load_loupe_prefetch());
                Task::batch(tasks)
            }

            Msg::LoupeGeometry { viewport, native } => {
                self.loupe.viewport = Some(viewport);
                self.loupe.native = Some(native);
                Task::none()
            }

            Msg::LoupeZoomActual => {
                // Toggle between fit and 1:1 (Lightroom-style). Centre-anchored.
                let intent = if self.loupe.zoom > super::super::LOUPE_ZOOM_MIN {
                    loupe::LoupeIntent::Reset
                } else {
                    loupe::LoupeIntent::ZoomTo {
                        level: loupe::ZoomLevel::Actual,
                        anchor: self.loupe_center(),
                    }
                };
                self.apply_loupe_intent(intent)
            }

            Msg::ToggleFullscreen => {
                self.fullscreen = !self.fullscreen;
                let mode = if self.fullscreen {
                    iced::window::Mode::Fullscreen
                } else {
                    iced::window::Mode::Windowed
                };
                iced::window::oldest().then(move |id| match id {
                    Some(id) => iced::window::set_mode(id, mode),
                    None => Task::none(),
                })
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
                        return Task::batch([
                            self.scroll_to_index(self.loupe.idx),
                            self.restore_sidebar_scroll(),
                        ]);
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
                            self.loupe.load_error = None;
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
                    ViewMode::People
                    | ViewMode::Compare
                    | ViewMode::ResolveStacks
                    | ViewMode::Settings => {}
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
                    return self.handle_navigation_msg(Msg::MouseRightClicked);
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

                // A drag that didn't travel past the threshold is a plain click. For
                // an album that means navigate to it (and collapse the selection);
                // for a photo it falls through to the tile click/loupe handling.
                if let Some(Drag { payload, past_threshold: false, .. }) = &drag {
                    if let DragPayload::Albums { pressed } = payload {
                        self.selected_albums.clear();
                        return Task::done(Msg::SidebarItemClicked(SidebarItem::Album(pressed.clone())));
                    }
                }

                // A real album drag resolves entirely here (drop on a shelf, or
                // cancel and keep the selection if released off any target).
                if let Some(Drag { payload: payload @ DragPayload::Albums { .. }, past_threshold: true, .. }) = &drag {
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

            Msg::EscapePressed => {
                if self.purge_pending.is_some() {
                    self.purge_pending = None;
                    return Task::none();
                }
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
                    return Task::batch([
                        self.scroll_to_index(self.loupe.idx),
                        self.restore_sidebar_scroll(),
                    ]);
                }
                if matches!(self.view_mode, ViewMode::ResolveStacks) {
                    return self.exit_resolve(false);
                }
                if matches!(self.view_mode, ViewMode::Compare | ViewMode::Settings) {
                    self.view_mode = ViewMode::Browse;
                    return self.restore_sidebar_scroll();
                }
                if !self.selected_albums.is_empty() {
                    self.selected_albums.clear();
                    return Task::none();
                }
                self.create_album_input = None;
                self.pending_album_shelf = None;
                self.rename_album_id = None;
                self.create_shelf_input = None;
                self.pending_shelf_albums.clear();
                self.rename_shelf_id = None;
                self.shelf_pending_delete = None;
                self.faces.rename_cluster_id = None;
                self.filters.save_smart_input = None;
                self.remove_from_album_pending = false;
                self.reject_delete_pending = false;
                self.purge_pending = None;
                Task::none()
            }

            Msg::Scrolled { y, height, width } => {
                self.scroll_y = y;
                self.viewport_height = height;
                self.viewport_width = width;
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
                return self.handle_navigation_msg(Msg::MouseRightClicked);
            }

            Msg::AlbumPressed(id) => {
                // Press-down on an album row (the row's `mouse_area` captured it).
                // Ctrl → menu, Cmd → toggle the multi-selection, plain → begin a
                // drag candidate whose click/drop is resolved on `MouseReleased`.
                self.context_menu = None;
                if self.modifiers.control() {
                    return self
                        .handle_navigation_msg(Msg::OpenSidebarEntityMenu(SidebarItem::Album(id)));
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
                    if matches!(&self.loupe.load_error, Some((e, _)) if *e == idx) {
                        self.loupe.load_error = None;
                    }
                }
                Task::none()
            }

            Msg::LoupeFullResFailed { idx, error } => {
                if self.loupe.idx == idx && matches!(self.view_mode, ViewMode::Loupe) {
                    self.loupe.load_error = Some((idx, error));
                }
                Task::none()
            }

            Msg::OpenPrivacySettings => {
                open_privacy_settings();
                Task::none()
            }

            Msg::LoupeHiresLoaded { idx, handle } => {
                if self.loupe.idx == idx && matches!(self.view_mode, ViewMode::Loupe) {
                    self.loupe.full_res = Some((idx, handle));
                    self.loupe.hires_loaded = true;
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
                // While an album multi-selection is active, Cmd+A expands it to all
                // sibling albums — every album sharing a shelf (or the ungrouped
                // top level) with something already selected, like Cmd+A within a
                // Finder folder. Otherwise it selects the whole grid.
                if !self.selected_albums.is_empty() {
                    self.selected_albums = album_siblings(&self.albums, &self.selected_albums);
                    return Task::none();
                }
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

            other => {
                debug_assert!(false, "handle_navigation_msg received misrouted message: {other:?}");
                Task::none()
            }
        }
    }

    pub(crate) fn load_loupe_full_res(&self) -> Task<Msg> {
        let idx = self.loupe.idx;
        let Some(file) = self.files.get(idx) else { return Task::none() };
        let path = file.disk_path();
        let filename = file.name.clone();
        let fallback_name = filename.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || match decode_image_for_display(&path, false) {
                    Some(handle) => Ok(handle),
                    None => Err(diagnose_load_failure(&path, &filename)),
                })
                .await
                .unwrap_or_else(|_| {
                    Err(LoupeLoadError {
                        filename: fallback_name,
                        message: "Decoding the image crashed.".into(),
                        permission: false,
                    })
                })
            },
            move |result| match result {
                Ok(handle) => Msg::LoupeFullResLoaded { idx, handle },
                Err(error) => Msg::LoupeFullResFailed { idx, error },
            },
        )
    }

    /// Full-demosaic decode for the current RAW, swapped in when the user zooms
    /// to 100% so the focus check is pixel-accurate. No-op for non-RAW (already
    /// full quality) or once already loaded for this photo.
    /// Live loupe geometry from the last hover-reported sizes, if known. Used
    /// for centre-anchored button/key zoom; the widget builds its own from the
    /// current layout for pointer-anchored gestures.
    fn loupe_geometry(&self) -> Option<loupe::LoupeGeometry> {
        match (self.loupe.viewport, self.loupe.native) {
            (Some(viewport), Some(native))
                if viewport.width > 0.0 && viewport.height > 0.0 && native.width > 0.0 =>
            {
                Some(loupe::LoupeGeometry { viewport, native })
            }
            _ => None,
        }
    }

    fn loupe_center(&self) -> Point {
        self.loupe_geometry().map(|g| g.center()).unwrap_or(Point::ORIGIN)
    }

    /// Apply a loupe intent app-side (buttons / keys) through the shared reducer,
    /// then load the hi-res decode if we ended up zoomed in. When geometry isn't
    /// known yet, falls back to a geometry-free approximation the widget will
    /// re-clamp on its next draw.
    fn apply_loupe_intent(&mut self, intent: loupe::LoupeIntent) -> Task<Msg> {
        let prev = self.loupe.zoom;
        let cur = loupe::LoupeZoom { zoom: self.loupe.zoom, offset: self.loupe.pan };
        let next = match self.loupe_geometry() {
            Some(geo) => geo.apply(cur, intent),
            None => fallback_apply(cur, intent),
        };
        self.loupe.zoom = next.zoom;
        self.loupe.pan = next.offset;
        if next.zoom <= super::super::LOUPE_ZOOM_MIN {
            // Back at fit: the next zoom-in must re-decode the hi-res image.
            self.loupe.hires_loaded = false;
        }
        if next.zoom > super::super::LOUPE_ZOOM_MIN && next.zoom != prev {
            return self.load_loupe_hires();
        }
        Task::none()
    }

    pub(crate) fn load_loupe_hires(&self) -> Task<Msg> {
        if self.loupe.hires_loaded {
            return Task::none();
        }
        let idx = self.loupe.idx;
        let Some(file) = self.files.get(idx) else { return Task::none() };
        if !is_raw_path(&file.path) {
            return Task::none();
        }
        let path = file.disk_path();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || decode_image_for_display(&path, true))
                    .await
                    .ok()
                    .flatten()
            },
            move |handle_opt| match handle_opt {
                Some(handle) => Msg::LoupeHiresLoaded { idx, handle },
                None => Msg::NoOp,
            },
        )
    }

    pub(crate) fn load_compare_slot(&self, slot: usize) -> Task<Msg> {
        let Some(file) = self.compare.files[slot].as_ref() else { return Task::none() };
        let path = file.disk_path();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || decode_image_for_display(&path, false))
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
                let path = file.disk_path();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || decode_image_for_display(&path, false))
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

    /// Turn a finished drag into its drop action, validated through the
    /// `drop_allowed` matrix. Returns `Task::none()` when released off any
    /// compatible target. The single dispatch point for every payload.
    fn resolve_drop(&self, payload: &DragPayload, target: Option<DropTarget>) -> Task<Msg> {
        let Some(target) = target else { return Task::none() };
        if !drop_allowed(payload, &target) {
            return Task::none();
        }
        match (payload, target) {
            (DragPayload::Photos { ids, .. }, DropTarget::Album(album_id)) => {
                Task::done(Msg::DroppedToAlbum(album_id, ids.iter().cloned().collect()))
            }
            (DragPayload::Albums { pressed }, DropTarget::Shelf(shelf_id)) => {
                let album_ids = dragged_albums(pressed, &self.selected_albums);
                Task::done(Msg::MoveAlbumsToShelf { album_ids, shelf_id: Some(shelf_id) })
            }
            _ => Task::none(),
        }
    }
}

/// Every album sharing a shelf (or the ungrouped top level) with something
/// already selected — what `Cmd+A` expands an album selection to, like Cmd+A
/// within a Finder folder. The set of "containers" is derived from the current
/// selection, then every album in those containers is selected.
pub(crate) fn album_siblings(
    albums: &[isomfolio_core::models::Album],
    selected: &HashSet<AlbumId>,
) -> HashSet<AlbumId> {
    let shelves: HashSet<Option<String>> = albums
        .iter()
        .filter(|a| selected.contains(&a.id))
        .map(|a| a.shelf_id.clone())
        .collect();
    albums
        .iter()
        .filter(|a| shelves.contains(&a.shelf_id))
        .map(|a| a.id.clone())
        .collect()
}

/// The albums a shelf drop applies to: the whole multi-selection when the
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
/// sidebar's drop-zone mounting. Shelf-into-shelf (once nested shelves land)
/// becomes one new arm here.
pub(crate) fn drop_allowed(payload: &DragPayload, target: &DropTarget) -> bool {
    matches!(
        (payload, target),
        (DragPayload::Photos { .. }, DropTarget::Album(_))
            | (DragPayload::Albums { .. }, DropTarget::Shelf(_))
    )
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
/// Move a grid cursor by one keypress. Horizontal moves (`dx`) are *linear* so
/// stepping right off the end of a row lands on the first tile of the next row
/// (and left off the start lands on the previous row's last tile) — Finder /
/// Lightroom behaviour. Vertical moves (`dy`) stay columnar, keeping the column
/// and clamping at the top/bottom rows. `total` is assumed > 0.
/// Geometry-free intent application for the brief window after the loupe opens
/// but before the widget has reported its size. Anchoring is impossible without
/// geometry, so this only sets the zoom and scales the pan toward centre; the
/// widget re-clamps to the image edges on its next draw.
fn fallback_apply(cur: loupe::LoupeZoom, intent: loupe::LoupeIntent) -> loupe::LoupeZoom {
    use loupe::{LoupeIntent, LoupeZoom, ZoomLevel, ZOOM_MAX, ZOOM_MIN};
    let scale_to = |to: f32, cur: LoupeZoom| {
        let to = to.clamp(ZOOM_MIN, ZOOM_MAX);
        if to <= ZOOM_MIN {
            LoupeZoom { zoom: ZOOM_MIN, offset: iced::Vector::ZERO }
        } else {
            LoupeZoom { zoom: to, offset: cur.offset * (to / cur.zoom) }
        }
    };
    match intent {
        LoupeIntent::ZoomAround { factor, .. } => scale_to(cur.zoom * factor, cur),
        // No geometry → no true 1:1; fall back to 2× (matches the old behaviour).
        LoupeIntent::ZoomTo { level: ZoomLevel::Actual, .. } => {
            LoupeZoom { zoom: 2.0_f32.clamp(ZOOM_MIN, ZOOM_MAX), offset: iced::Vector::ZERO }
        }
        LoupeIntent::ZoomTo { level: ZoomLevel::Fit, .. } | LoupeIntent::Reset => {
            LoupeZoom { zoom: ZOOM_MIN, offset: iced::Vector::ZERO }
        }
        LoupeIntent::PanTo(offset) => LoupeZoom { zoom: cur.zoom, offset },
    }
}

fn grid_step(current: i32, dx: i32, dy: i32, cols: i32, total: i32) -> i32 {
    if dx != 0 {
        (current + dx).clamp(0, total - 1)
    } else {
        let (row, col) = (current / cols, current % cols);
        let new_row = (row + dy).clamp(0, (total - 1) / cols);
        (new_row * cols + col).min(total - 1)
    }
}

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

pub(crate) fn is_raw_path(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    isomfolio_core::indexing::thumbnail::is_raw_extension(ext)
}

pub(crate) fn decode_image_for_display(path: &str, prefer_full: bool) -> Option<iced::widget::image::Handle> {
    let img = open_image(path, prefer_full)?;
    let rgba = img.into_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    Some(iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw()))
}

/// Classify why a full-res decode produced nothing, into a user-facing reason +
/// a `permission` flag that drives the resolution action. Distinguishes a
/// permission denial (macOS TCC on a protected folder) from a missing file or an
/// unsupported/corrupt image by probing the raw file open.
fn diagnose_load_failure(path: &str, filename: &str) -> LoupeLoadError {
    use std::io::ErrorKind;
    let (message, permission) = match std::fs::File::open(path) {
        Err(e) if e.kind() == ErrorKind::PermissionDenied => (
            "macOS blocked access to this file. It's in a protected folder \
             (Downloads, Desktop, Documents). Grant the app Full Disk Access, \
             then reopen the photo."
                .to_string(),
            true,
        ),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            ("The file is no longer at its expected location.".to_string(), false)
        }
        Err(e) => (format!("Couldn't open the file: {e}."), false),
        // Opened fine, so the decoder rejected the contents.
        Ok(_) => ("The image data is unsupported or corrupt.".to_string(), false),
    };
    LoupeLoadError { filename: filename.to_string(), message, permission }
}

/// Open the OS privacy pane where file-access is granted. macOS deep-links to
/// Full Disk Access; other platforms have no equivalent one-click target.
fn open_privacy_settings() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
            .spawn();
    }
}

/// Decode an image for on-screen display. For RAW, `prefer_full = false` returns
/// the embedded preview first (fast — used for fit-to-window browsing and
/// prefetch), and only falls back to a full demosaic if no preview exists.
/// `prefer_full = true` does the full demosaic (used when zoomed to 100% for a
/// pixel-accurate focus check). Non-RAW formats ignore the flag.
fn open_image(path: &str, prefer_full: bool) -> Option<image::DynamicImage> {
    use rawler::decoders::RawDecodeParams;
    use rawler::rawsource::RawSource;
    use std::path::Path;

    if is_raw_path(path) {
        let source = RawSource::new(Path::new(path)).ok()?;
        let decoder = rawler::get_decoder(&source).ok()?;
        let params = RawDecodeParams::default();
        let full = || decoder.full_image(&source, &params).ok().flatten();
        let preview = || decoder.preview_image(&source, &params).ok().flatten();
        return if prefer_full {
            full().or_else(preview)
        } else {
            preview().or_else(full)
        };
    }

    match image::open(path) {
        Ok(img) => Some(img),
        Err(e) => {
            // Common cause on macOS: the file is in a TCC-protected folder
            // (~/Downloads, ~/Desktop, ~/Documents) the app lacks access to —
            // the read fails with "Operation not permitted". The loupe then
            // falls back to the cached thumbnail (looks pixelated) and can't zoom.
            eprintln!("[loupe] cannot read {path}: {e}");
            None
        }
    }
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
    use isomfolio_core::models::Album;

    mod diagnose_load_failure_fn {
        use super::*;

        fn temp_path(name: &str) -> std::path::PathBuf {
            let p = std::env::temp_dir().join(format!(
                "isomfolio-test-{}-{name}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            p
        }

        #[test]
        fn missing_file_is_not_a_permission_error() {
            let err = diagnose_load_failure("/no/such/file.jpg", "file.jpg");
            assert!(!err.permission);
            assert_eq!(err.filename, "file.jpg");
            assert!(err.message.to_lowercase().contains("location"));
        }

        #[test]
        fn readable_but_undecodable_file_reports_corrupt_not_permission() {
            let path = temp_path("corrupt.jpg");
            std::fs::write(&path, b"definitely not an image").unwrap();
            let err = diagnose_load_failure(path.to_str().unwrap(), "x.jpg");
            let _ = std::fs::remove_file(&path);
            assert!(!err.permission);
            let m = err.message.to_lowercase();
            assert!(m.contains("unsupported") || m.contains("corrupt"), "got: {}", err.message);
        }

        #[cfg(unix)]
        #[test]
        fn unreadable_file_is_flagged_as_permission() {
            use std::os::unix::fs::PermissionsExt;
            let path = temp_path("denied.jpg");
            std::fs::write(&path, b"x").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
            // Running as root bypasses the mode bits; only assert when truly denied.
            let denied = std::fs::File::open(&path).is_err();
            let err = diagnose_load_failure(path.to_str().unwrap(), "x.jpg");
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644));
            let _ = std::fs::remove_file(&path);
            if denied {
                assert!(err.permission, "expected permission flag, got: {}", err.message);
            }
        }
    }

    mod grid_step_nav {
        use super::*;

        // 6 items, 3 columns:  [0 1 2 / 3 4 5]
        const COLS: i32 = 3;
        const TOTAL: i32 = 6;

        #[test]
        fn right_at_row_end_wraps_to_next_row_start() {
            assert_eq!(grid_step(2, 1, 0, COLS, TOTAL), 3);
        }

        #[test]
        fn left_at_row_start_wraps_to_prev_row_end() {
            assert_eq!(grid_step(3, -1, 0, COLS, TOTAL), 2);
        }

        #[test]
        fn right_at_last_tile_stays_clamped() {
            assert_eq!(grid_step(5, 1, 0, COLS, TOTAL), 5);
        }

        #[test]
        fn down_keeps_column_and_clamps_at_bottom() {
            assert_eq!(grid_step(1, 0, 1, COLS, TOTAL), 4);
            assert_eq!(grid_step(4, 0, 1, COLS, TOTAL), 4);
        }

        #[test]
        fn up_keeps_column_and_clamps_at_top() {
            assert_eq!(grid_step(4, 0, -1, COLS, TOTAL), 1);
            assert_eq!(grid_step(1, 0, -1, COLS, TOTAL), 1);
        }

        #[test]
        fn down_into_short_last_row_clamps_to_last_tile() {
            // 5 items, 3 cols: [0 1 2 / 3 4]; down from col 2 has no tile, clamp.
            assert_eq!(grid_step(2, 0, 1, 3, 5), 4);
        }
    }

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

        fn album(id: &str, shelf: Option<&str>) -> Album {
            Album {
                id: id.to_string(),
                name: id.to_string(),
                kind: AlbumKind::Manual,
                sort_order: 0,
                shelf_id: shelf.map(|s| s.to_string()),
            }
        }

        fn set(items: &[&str]) -> HashSet<AlbumId> {
            items.iter().map(|s| s.to_string()).collect()
        }

        #[test]
        fn selecting_one_album_expands_to_its_whole_shelf() {
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
        fn selection_spanning_two_shelves_expands_to_both() {
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

    mod drop_compat {
        use super::*;
        use std::collections::HashSet;

        fn photos() -> DragPayload {
            DragPayload::Photos { origin_idx: 0, ids: HashSet::new() }
        }
        fn albums() -> DragPayload {
            DragPayload::Albums { pressed: "a".into() }
        }

        #[test]
        fn photos_drop_onto_albums_only() {
            assert!(drop_allowed(&photos(), &DropTarget::Album("a".into())));
            assert!(!drop_allowed(&photos(), &DropTarget::Shelf("s".into())));
        }

        #[test]
        fn albums_drop_onto_shelves_only() {
            assert!(drop_allowed(&albums(), &DropTarget::Shelf("s".into())));
            assert!(!drop_allowed(&albums(), &DropTarget::Album("a".into())));
        }
    }
}
