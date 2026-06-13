use iced::Task;

use super::super::{App, Msg, ViewMode, TILE_SIZE_MAX, TILE_SIZE_MIN, TILE_SIZE_STEP};
use super::drag_drop::album_siblings;
use super::pointer::range_select;

impl App {
    pub(super) fn handle_navigation_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::TileSizeUp => {
                // In the loupe the "zoom" keys zoom the image, not the grid tiles.
                if matches!(self.view_mode, ViewMode::Loupe) {
                    return self.handle_loupe_msg(Msg::LoupeZoomBy(1.25));
                }
                self.tile_px = (self.tile_px + TILE_SIZE_STEP).min(TILE_SIZE_MAX);
                if let Some(idx) = self.anchor_idx { self.scroll_to_index(idx) } else { Task::none() }
            }

            Msg::TileSizeDown => {
                if matches!(self.view_mode, ViewMode::Loupe) {
                    return self.handle_loupe_msg(Msg::LoupeZoomBy(0.8));
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
                if matches!(self.view_mode, ViewMode::ResolveStacks) {
                    let step = dx + dy;
                    // In Grid, arrows step between groups. In Strip/Full, arrows move
                    // the focused frame *within* the group (group nav is Shift+arrow,
                    // handled in NavigateExtend).
                    if matches!(self.resolve.layout, crate::app::SurfaceLayout::Grid) {
                        return if step < 0 {
                            self.handle_stacking_msg(Msg::ResolvePrevStack)
                        } else if step > 0 {
                            self.handle_stacking_msg(Msg::ResolveSkipStack)
                        } else {
                            Task::none()
                        };
                    }
                    let n = self
                        .resolve
                        .stacks
                        .get(self.resolve.idx)
                        .map_or(0, |s| s.frames.len());
                    if n > 0 && step != 0 {
                        // Clamp, don't wrap — the first/last frame never loops.
                        self.resolve.focus =
                            (self.resolve.focus as i32 + step).clamp(0, n as i32 - 1) as usize;
                    }
                    return Task::none();
                }
                if matches!(self.view_mode, ViewMode::Loupe | ViewMode::Preview) {
                    let total = self.files.len();
                    if total == 0 {
                        return Task::none();
                    }
                    let delta = dx + dy;
                    // Clamp, don't wrap — stepping off either end stays put. A scoped
                    // loupe (a multi-selection sent to review) steps between scope
                    // entries; otherwise between all files.
                    let scoped =
                        matches!(self.view_mode, ViewMode::Loupe) && !self.loupe.scope.is_empty();
                    let new_idx = if scoped {
                        let scope = &self.loupe.scope;
                        let pos = scope.iter().position(|&i| i == self.loupe.idx).unwrap_or(0);
                        let npos = (pos as i32 + delta).clamp(0, scope.len() as i32 - 1) as usize;
                        scope[npos]
                    } else {
                        (self.loupe.idx as i32 + delta).clamp(0, total as i32 - 1) as usize
                    };
                    if new_idx == self.loupe.idx {
                        // At the boundary: a true no-op, no needless reload/flicker.
                        return Task::none();
                    }
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
                // In Sift, Shift+arrow always steps between groups, whatever the
                // layout (plain arrows may be moving frame focus instead).
                if matches!(self.view_mode, ViewMode::ResolveStacks) {
                    let step = dx + dy;
                    return if step < 0 {
                        self.handle_stacking_msg(Msg::ResolvePrevStack)
                    } else if step > 0 {
                        self.handle_stacking_msg(Msg::ResolveSkipStack)
                    } else {
                        Task::none()
                    };
                }
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
                    return self.exit_loupe_to_grid();
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
                self.pending_album_group = None;
                self.rename_album_id = None;
                self.create_group_input = None;
                self.pending_group_albums.clear();
                self.pending_group_parent = None;
                self.rename_group_id = None;
                self.group_pending_delete = None;
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
                // As the viewport moves, pull the now-visible rows to the front of
                // the thumbnail queue so they generate ahead of the rest of the
                // backlog (the "scroll, then wait forever" complaint). Gated to
                // fire only when the visible row window actually shifts and work
                // remains, so a flick-scroll doesn't spam the worker coordinator.
                if self.thumb_ctx.pending > 0 {
                    let step = self.row_step();
                    let row = if step > 0.0 {
                        (((self.scroll_y - crate::app::GRID_PADDING) / step) as usize)
                            .saturating_sub(crate::app::BUFFER_ROWS)
                    } else {
                        0
                    };
                    if row != self.thumb_priority_row {
                        self.thumb_priority_row = row;
                        let ids = self.visible_file_ids();
                        if let Some(pool) = &self.thumb_ctx.pool {
                            pool.prioritize(&ids);
                        }
                    }
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

            Msg::SelectAll => {
                // While an album multi-selection is active, Cmd+A expands it to all
                // sibling albums — every album sharing a group (or the ungrouped
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
}


/// Move a grid cursor by one keypress. Horizontal moves (`dx`) are *linear* so
/// stepping right off the end of a row lands on the first tile of the next row
/// (and left off the start lands on the previous row's last tile) — Finder /
/// Lightroom behaviour. Vertical moves (`dy`) stay columnar, keeping the column
/// and clamping at the top/bottom rows. `total` is assumed > 0.

fn grid_step(current: i32, dx: i32, dy: i32, cols: i32, total: i32) -> i32 {
    if dx != 0 {
        (current + dx).clamp(0, total - 1)
    } else {
        let (row, col) = (current / cols, current % cols);
        let new_row = (row + dy).clamp(0, (total - 1) / cols);
        (new_row * cols + col).min(total - 1)
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

}
