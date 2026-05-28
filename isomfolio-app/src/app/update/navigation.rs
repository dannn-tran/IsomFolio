use iced::Task;

use super::super::{
    App, ContextMenuState, ContextMenuTarget, DragState, LoupeState, Msg, SidebarItem, ViewMode,
    ALBUM_ITEM_HEIGHT, ALBUM_ROW_GAP, SIDEBAR_ALBUMS_BASE_Y, SIDEBAR_HANDLE_WIDTH,
    SIDEBAR_WIDTH_MAX, SIDEBAR_WIDTH_MIN, TILE_SIZE_MAX, TILE_SIZE_MIN, TILE_SIZE_STEP,
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
                    let mut tasks =
                        vec![self.load_loupe_full_res(), self.load_loupe_prefetch()];
                    if matches!(self.view_mode, ViewMode::Preview) {
                        tasks.push(self.scroll_to_index(new_idx));
                        tasks.push(self.maybe_load_detail());
                    }
                    if let Some(handle) = self.loupe.prefetch.remove(&new_idx) {
                        self.loupe.full_res = Some((new_idx, handle));
                        return Task::batch(tasks);
                    }
                    self.loupe.full_res = None;
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
                self.grid_selected.clear();
                if let Some(f) = self.files.get(new_idx) {
                    self.grid_selected.insert(f.id.clone());
                }
                let scroll = self.scroll_to_index(new_idx);
                let detail = self.maybe_load_detail();
                Task::batch([scroll, detail])
            }

            Msg::OpenLoupe => {
                match self.view_mode {
                    ViewMode::Loupe => {
                        self.anchor_idx = Some(self.loupe.idx);
                        self.grid_selected.clear();
                        if let Some(f) = self.files.get(self.loupe.idx) {
                            self.grid_selected.insert(f.id.clone());
                        }
                        self.view_mode = ViewMode::Browse;
                        self.loupe.full_res = None;
                        self.loupe.prefetch.clear();
                        return self.scroll_to_index(self.loupe.idx);
                    }
                    ViewMode::Preview => {
                        self.view_mode = ViewMode::Loupe;
                        return Task::none();
                    }
                    ViewMode::Browse => {
                        if !self.files.is_empty() {
                            let idx =
                                self.anchor_idx.unwrap_or(0).min(self.files.len() - 1);
                            self.loupe.idx = idx;
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
                    ViewMode::People | ViewMode::Compare => {}
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
                if self.drag.state.as_ref().map_or(false, |d| d.active) {
                    if pos.x < self.sidebar_width + SIDEBAR_HANDLE_WIDTH {
                        let n_folders = self.folders.len();
                        let albums_top =
                            SIDEBAR_ALBUMS_BASE_Y + n_folders as f32 * (ALBUM_ITEM_HEIGHT + ALBUM_ROW_GAP);
                        let y_in_content = pos.y + self.sidebar_scroll_y - albums_top;
                        let row_h = ALBUM_ITEM_HEIGHT + ALBUM_ROW_GAP;
                        self.drag.hover_album = if y_in_content >= 0.0 {
                            let idx = (y_in_content / row_h) as usize;
                            self.albums.get(idx).and_then(|a| {
                                if matches!(a.kind, isomfolio_core::models::AlbumKind::Manual) {
                                    Some(a.id.clone())
                                } else {
                                    None
                                }
                            })
                        } else {
                            None
                        };
                    } else {
                        self.drag.hover_album = None;
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
                            self.anchor_idx = None;
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
                if matches!(self.view_mode, ViewMode::Compare | ViewMode::Loupe) {
                    self.view_mode = ViewMode::Browse;
                    return Task::none();
                }
                self.create_album_input = None;
                self.rename_album_id = None;
                self.faces.rename_cluster_id = None;
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
                if self.anchor_idx.is_none() && !self.files.is_empty() {
                    self.anchor_idx = Some(0);
                }
                Task::none()
            }

            Msg::DeselectAll => {
                self.grid_selected.clear();
                self.anchor_idx = None;
                Task::none()
            }

            Msg::OpenCompare => {
                if self.grid_selected.len() != 2 {
                    self.status = "Select exactly 2 photos to compare".to_string();
                    return Task::none();
                }
                let mut sel = self.grid_selected.iter();
                let id0 = sel.next().unwrap().clone();
                let id1 = sel.next().unwrap().clone();
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

            Msg::ShowInFinder(path) => {
                self.context_menu = None;
                let _ = std::process::Command::new("open").arg("-R").arg(&path).spawn();
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
                tokio::task::spawn_blocking(move || {
                    image::open(&path).ok().map(|img| {
                        let rgba = img.into_rgba8();
                        let (w, h) = (rgba.width(), rgba.height());
                        iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw())
                    })
                })
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
                tokio::task::spawn_blocking(move || {
                    image::open(&path).ok().map(|img| {
                        let rgba = img.into_rgba8();
                        let (w, h) = (rgba.width(), rgba.height());
                        iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw())
                    })
                })
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
                        tokio::task::spawn_blocking(move || {
                            image::open(&path).ok().map(|img| {
                                let rgba = img.into_rgba8();
                                let (w, h) = (rgba.width(), rgba.height());
                                iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw())
                            })
                        })
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
}
