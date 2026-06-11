use std::collections::HashMap;
use std::time::UNIX_EPOCH;

use iced::Task;
use isomfolio_core::indexing::thumbnail::thumbnail_cache_path;
use isomfolio_core::models::{AssetFile, Flag, SearchQuery};
use isomfolio_core::phash;

use super::loupe_load::decode_image_for_display;
use super::LockUnwrap;
use super::super::{App, Msg, ResolveState, SidebarItem, StackReview, UndoOp, ViewMode};

impl App {
    pub(super) fn handle_stacking_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::RunStacking => self.run_stacking_task(),

            Msg::RestackNow => {
                self.stacking_manual = true;
                self.run_stacking_task()
            }

            Msg::StacksUpdated => {
                self.stacking_in_flight = false;
                // When collapsed, the visible file set changes (one tile per
                // stack), so the list must reload; otherwise only the ⧉ badges do.
                let refresh = if self.collapse_bursts {
                    self.load_files_task()
                } else {
                    self.load_file_side_data_task()
                };
                Task::batch([refresh, self.load_stack_stats_task()])
            }

            Msg::StackStatsLoaded(stats) => {
                self.stack_stats = stats;
                // Only a user-initiated re-stack announces; auto passes (which
                // fire repeatedly during import) update the count silently.
                if std::mem::take(&mut self.stacking_manual) {
                    self.status = if stats.stacks > 0 {
                        format!(
                            "Stacking complete — {} stack{} across {} frames",
                            stats.stacks,
                            if stats.stacks == 1 { "" } else { "s" },
                            stats.stacked_frames,
                        )
                    } else {
                        "Stacking complete — no near-duplicate stacks found".to_string()
                    };
                }
                Task::none()
            }

            Msg::StackKeepOnly(anchor) => self.cull_stack(anchor, true),
            Msg::StackRejectAll(anchor) => self.cull_stack(anchor, false),

            Msg::ToggleStackExpanded(file_id) => {
                self.context_menu = None;
                let Some(burst) = self.file_burst_ids.get(&file_id).cloned() else {
                    return Task::none();
                };
                if !self.expanded_bursts.remove(&burst) {
                    self.expanded_bursts.insert(burst);
                }
                self.load_files_task()
            }

            Msg::StackFlagsApplied { before, after, kept } => {
                if before.is_empty() {
                    return Task::none();
                }
                let total = before.len();
                self.push_undo(crate::app::UndoOp::Flags { before, after });
                self.status = if kept > 0 {
                    format!("Kept {kept}, rejected {} in stack", total - kept)
                } else {
                    format!("Rejected {total} in stack")
                };
                // Reload so the rep badge (and any expanded members) reflect the
                // new flags; the undo snapshot already covers hidden siblings.
                self.load_files_task()
            }

            Msg::OpenResolveStacks => self.open_resolve_stacks_task(),

            Msg::ResolveStacksLoaded(stacks) => {
                if stacks.is_empty() {
                    self.status = "No stacks to review in this view".to_string();
                    return Task::none();
                }
                self.resolve = ResolveState { stacks, idx: 0, ..Default::default() };
                self.view_mode = ViewMode::ResolveStacks;
                // Drop any grid selection so flag/delete keys (which target the
                // selection) are inert while reviewing — clicks pick keepers here.
                self.grid_selected.clear();
                self.enter_resolve_stack(0)
            }

            Msg::ResolveFrameLoaded { stack_idx, frame_idx, handle } => {
                if matches!(self.view_mode, ViewMode::ResolveStacks) && self.resolve.idx == stack_idx {
                    self.resolve.handles.insert(frame_idx, handle);
                }
                Task::none()
            }

            Msg::ToggleResolveKeeper(id) => {
                if !self.resolve.keepers.remove(&id) {
                    self.resolve.keepers.insert(id);
                }
                Task::none()
            }

            Msg::ResolveSkipStack => self.advance_resolve(self.resolve.idx + 1),

            Msg::ResolvePrevStack => {
                let prev = self.resolve.idx.saturating_sub(1);
                self.enter_resolve_stack(prev)
            }

            Msg::ResolveApplyAndNext | Msg::ResolveConfirm => self.apply_resolve_and_advance(),

            Msg::ResolveResetAuto => self.resolve_reset_auto(),

            Msg::ResolveFinished => self.exit_resolve(true),

            _ => Task::none(),
        }
    }

    /// Load the at-rest stacking summary for the Settings panel. Cheap (three
    /// COUNT queries); fired after each pass and on catalog open.
    pub(crate) fn load_stack_stats_task(&self) -> Task<Msg> {
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                g.stack_stats().unwrap_or_default()
            },
            Msg::StackStatsLoaded,
        )
    }

    /// Build the review queue: every multi-frame stack in the current view, in
    /// capture order, each tagged with its sharpest frame as the default keeper.
    /// Runs the scope's search uncollapsed so all members are present.
    fn open_resolve_stacks_task(&self) -> Task<Msg> {
        let Some(catalog) = self.catalog.clone() else {
            return Task::none();
        };
        let item = self.selected_item.clone();
        let mut query = self.build_search_query();
        query.collapse_bursts = false;
        query.expanded_bursts = Vec::new();
        let is_smart = self.current_album_is_smart();

        Task::perform(
            async move {
                let cat = catalog.lock_unwrap();
                let files = match item {
                    SidebarItem::AllFiles => cat.search(&query).unwrap_or_default(),
                    SidebarItem::Folder(path) => {
                        let q = SearchQuery { folder_path: Some(path), folder_recursive: true, ..query };
                        cat.search(&q).unwrap_or_default()
                    }
                    SidebarItem::Album(album_id) => {
                        if is_smart {
                            cat.search(&query).unwrap_or_default()
                        } else {
                            cat.search_manual_album(&album_id, &query).unwrap_or_default()
                        }
                    }
                    SidebarItem::Import(batch_id) => {
                        let q = SearchQuery { import_batch: Some(batch_id), ..query };
                        cat.search(&q).unwrap_or_default()
                    }
                    SidebarItem::Deleted => Vec::new(), // no stacking in the Deleted view
                };
                let ids: Vec<String> = files.iter().map(|f| f.id.clone()).collect();
                let membership = cat.get_stack_membership(&ids).unwrap_or_default();
                group_stacks_for_review(files, &membership)
            },
            Msg::ResolveStacksLoaded,
        )
    }

    /// Show stack `i`: reset keepers to its sharpest frame and kick off a full-res
    /// decode for each frame. Exits the mode if `i` is past the end.
    pub(crate) fn enter_resolve_stack(&mut self, i: usize) -> Task<Msg> {
        let Some(stack) = self.resolve.stacks.get(i) else {
            return self.exit_resolve(true);
        };
        self.resolve.idx = i;
        self.resolve.handles.clear();
        self.resolve.keepers = std::iter::once(stack.rep_id.clone()).collect();
        let tasks: Vec<Task<Msg>> = stack
            .frames
            .iter()
            .enumerate()
            .map(|(frame_idx, f)| {
                let path = f.disk_path();
                let stack_idx = i;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || decode_image_for_display(&path, false))
                            .await
                            .ok()
                            .flatten()
                    },
                    move |h| match h {
                        Some(handle) => Msg::ResolveFrameLoaded { stack_idx, frame_idx, handle },
                        None => Msg::NoOp,
                    },
                )
            })
            .collect();
        Task::batch(tasks)
    }

    fn advance_resolve(&mut self, next: usize) -> Task<Msg> {
        if next < self.resolve.stacks.len() {
            self.enter_resolve_stack(next)
        } else {
            self.exit_resolve(true)
        }
    }

    pub(super) fn in_resolve(&self) -> bool {
        matches!(self.view_mode, ViewMode::ResolveStacks)
    }

    /// Toggle the keeper mark on the `n`-th frame (1-based) of the current stack —
    /// the number-key shortcut in the review. Out-of-range numbers are ignored.
    pub(super) fn resolve_toggle_frame(&mut self, n: usize) -> Task<Msg> {
        if let Some(stack) = self.resolve.stacks.get(self.resolve.idx) {
            if let Some(f) = stack.frames.get(n.wrapping_sub(1)) {
                let id = f.id.clone();
                if !self.resolve.keepers.remove(&id) {
                    self.resolve.keepers.insert(id);
                }
            }
        }
        Task::none()
    }

    /// Restore the current stack's keepers to the auto-picked (sharpest) frame.
    pub(super) fn resolve_reset_auto(&mut self) -> Task<Msg> {
        if let Some(stack) = self.resolve.stacks.get(self.resolve.idx) {
            self.resolve.keepers = std::iter::once(stack.rep_id.clone()).collect();
        }
        Task::none()
    }

    /// Flag the current stack (keepers → Pick, the rest → Reject), record undo,
    /// then advance. The flag write runs off-thread; when it's the *last* stack we
    /// chain the exit reload onto it so the grid reflects the final write.
    fn apply_resolve_and_advance(&mut self) -> Task<Msg> {
        let Some(stack) = self.resolve.stacks.get(self.resolve.idx) else {
            return Task::none();
        };
        // Keeping nothing would silently reject the whole stack — almost never the
        // intent. Treat it as a skip and nudge the user, rather than blow it away.
        if self.resolve.keepers.is_empty() {
            self.status = "Nothing kept — skipped (pick at least one frame to keep)".to_string();
            return self.advance_resolve(self.resolve.idx + 1);
        }
        let keepers = self.resolve.keepers.clone();
        let picks: Vec<String> = stack
            .frames
            .iter()
            .filter(|f| keepers.contains(&f.id))
            .map(|f| f.id.clone())
            .collect();
        let rejects: Vec<String> = stack
            .frames
            .iter()
            .filter(|f| !keepers.contains(&f.id))
            .map(|f| f.id.clone())
            .collect();
        let before: Vec<(String, Flag)> = stack.frames.iter().map(|f| (f.id.clone(), f.flag)).collect();
        let after: Vec<(String, Flag)> = stack
            .frames
            .iter()
            .map(|f| (f.id.clone(), if keepers.contains(&f.id) { Flag::Pick } else { Flag::Reject }))
            .collect();
        self.push_undo(UndoOp::Flags { before, after });

        let Some(conn) = self.catalog.clone() else { return Task::none() };
        let is_last = self.resolve.idx + 1 >= self.resolve.stacks.len();
        let write = Task::perform(
            async move {
                let g = conn.lock_unwrap();
                if !picks.is_empty() {
                    let _ = g.set_files_flag(&picks, Flag::Pick);
                }
                if !rejects.is_empty() {
                    let _ = g.set_files_flag(&rejects, Flag::Reject);
                }
            },
            move |_| if is_last { Msg::ResolveFinished } else { Msg::NoOp },
        );
        if is_last {
            // Stay on the last stack until its write lands (ResolveFinished exits).
            write
        } else {
            Task::batch([write, self.enter_resolve_stack(self.resolve.idx + 1)])
        }
    }

    /// Leave the review. `completed` distinguishes finishing the queue from an
    /// Esc bail-out (only the message differs). Reloads the grid so all the flag
    /// changes made during the pass are reflected.
    pub(crate) fn exit_resolve(&mut self, completed: bool) -> Task<Msg> {
        self.view_mode = ViewMode::Browse;
        let was_scenes = self.resolve.scenes;
        self.resolve = ResolveState::default();
        if completed {
            self.status = if was_scenes { "Scene review complete" } else { "Stack review complete" }.to_string();
        }
        Task::batch([self.load_files_task(), self.restore_sidebar_scroll()])
    }

    /// Cull a whole stack from its anchor frame: `keep_one` keeps the anchor as a
    /// Pick and rejects the rest; otherwise every member is rejected. The flag
    /// write (and the prior-flag snapshot for undo) runs off the UI thread.
    fn cull_stack(&mut self, anchor: String, keep_one: bool) -> Task<Msg> {
        self.context_menu = None;
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        // `set_stack_flags` applies "anchor → Pick (if keep_one), rest → Reject" and
        // returns the prior flags; mirror that rule to reconstruct the `after` side
        // for a self-contained undo step (the anchor isn't in scope post-await).
        let anchor_for_after = anchor.clone();
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                g.set_stack_flags(&anchor, keep_one).map_err(|e| e.to_string())
            },
            move |res| match res {
                Ok(before) => {
                    let after = before
                        .iter()
                        .map(|(id, _)| {
                            let flag = if keep_one && *id == anchor_for_after {
                                Flag::Pick
                            } else {
                                Flag::Reject
                            };
                            (id.clone(), flag)
                        })
                        .collect();
                    Msg::StackFlagsApplied {
                        before,
                        after,
                        kept: if keep_one { 1 } else { 0 },
                    }
                }
                Err(e) => Msg::DbError(e),
            },
        )
    }

    /// Hash any not-yet-hashed files from their cached thumbnails, then re-derive
    /// per-folder stacks. Lock discipline: the file list is read under the catalog
    /// lock, thumbnails are decoded and hashed *unlocked*, then results are written
    /// back under the lock — never decode while holding the mutex.
    pub(crate) fn run_stacking_task(&mut self) -> Task<Msg> {
        if self.stacking_in_flight {
            return Task::none();
        }
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        self.stacking_in_flight = true;
        let catalog_dir = self.catalog_dir.clone();
        let threshold = self.app_settings.stack_threshold;
        let window = self.app_settings.stack_window_secs;

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let needing = {
                        let g = conn.lock_unwrap();
                        g.files_needing_phash().unwrap_or_default()
                    };

                    let mut rows: Vec<(String, u64, f64, i64)> = Vec::new();
                    for id in needing {
                        let path = thumbnail_cache_path(&catalog_dir, &id);
                        let p = std::path::Path::new(&path);
                        if !p.exists() {
                            continue; // No thumbnail yet — hash on a later pass.
                        }
                        let mtime = std::fs::metadata(p)
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        if let Ok(img) = image::open(p) {
                            rows.push((id, phash::dhash(&img), phash::sharpness(&img), mtime));
                        }
                    }

                    let g = conn.lock_unwrap();
                    if !rows.is_empty() {
                        if let Err(e) = g.store_phashes(&rows) {
                            eprintln!("[stack] store_phashes failed: {e}");
                        }
                    }
                    if let Err(e) = g.detect_and_store_stacks_all(threshold, window) {
                        eprintln!("[stack] detect_and_store_stacks_all failed: {e}");
                    }
                })
                .await
                .ok();
            },
            |_| Msg::StacksUpdated,
        )
    }
}

/// Group a capture-ordered file list into review stacks using each file's
/// `(burst_id, sharpness)`. Stacks keep first-appearance order; only groups of
/// ≥2 frames are returned, each tagged with its sharpest frame as `rep_id`.
fn group_stacks_for_review(
    files: Vec<AssetFile>,
    membership: &HashMap<String, (String, f64)>,
) -> Vec<StackReview> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<AssetFile>> = HashMap::new();
    for f in files {
        let Some((burst, _)) = membership.get(&f.id) else { continue };
        if !groups.contains_key(burst) {
            order.push(burst.clone());
        }
        groups.entry(burst.clone()).or_default().push(f);
    }
    order
        .into_iter()
        .filter_map(|burst| {
            let frames = groups.remove(&burst)?;
            if frames.len() < 2 {
                return None;
            }
            let rep_id = frames
                .iter()
                .max_by(|a, b| {
                    let sa = membership.get(&a.id).map(|(_, s)| *s).unwrap_or(0.0);
                    let sb = membership.get(&b.id).map(|(_, s)| *s).unwrap_or(0.0);
                    sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|f| f.id.clone())
                .unwrap_or_default();
            Some(StackReview { frames, rep_id })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: &str) -> AssetFile {
        AssetFile {
            id: id.to_string(),
            path: format!("/p/{id}.jpg"),
            name: format!("{id}.jpg"),
            folder: "/p".to_string(),
            folder_display: "/p".to_string(),
            ext: "jpg".to_string(),
            size_bytes: 1,
            mtime_unix: 0,
            created_at_unix: 0,
            is_orphaned: false,
            orphaned_at: None,
            flag: Flag::Unflagged,
            exif_date_unix: Some(0),
            gps_lat: None,
            gps_lon: None,
        }
    }

    mod group_stacks_for_review_fn {
        use super::*;

        fn membership(pairs: &[(&str, &str, f64)]) -> HashMap<String, (String, f64)> {
            pairs.iter().map(|(id, b, s)| (id.to_string(), (b.to_string(), *s))).collect()
        }

        #[test]
        fn keeps_only_multi_frame_stacks_in_capture_order() {
            let files = vec![file("a1"), file("a2"), file("solo"), file("b1"), file("b2")];
            let m = membership(&[
                ("a1", "A", 5.0), ("a2", "A", 9.0),
                ("b1", "B", 1.0), ("b2", "B", 2.0),
                // "solo" absent from membership → not stacked
            ]);
            let stacks = group_stacks_for_review(files, &m);
            assert_eq!(stacks.len(), 2);
            assert_eq!(stacks[0].frames.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), vec!["a1", "a2"]);
            assert_eq!(stacks[1].frames.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), vec!["b1", "b2"]);
        }

        #[test]
        fn rep_is_the_sharpest_frame() {
            let files = vec![file("a1"), file("a2"), file("a3")];
            let m = membership(&[("a1", "A", 5.0), ("a2", "A", 9.0), ("a3", "A", 1.0)]);
            let stacks = group_stacks_for_review(files, &m);
            assert_eq!(stacks[0].rep_id, "a2");
        }

        #[test]
        fn drops_singletons() {
            let files = vec![file("a1"), file("b1")];
            let m = membership(&[("a1", "A", 5.0), ("b1", "B", 5.0)]);
            assert!(group_stacks_for_review(files, &m).is_empty());
        }
    }

    mod resolve_keeper_keys {
        use super::*;
        use std::collections::HashSet;

        fn review_app() -> App {
            let mut app = App::new(None).0;
            let frames = vec![file("f1"), file("f2"), file("f3")];
            app.resolve = ResolveState {
                stacks: vec![StackReview { frames, rep_id: "f2".to_string() }],
                idx: 0,
                keepers: std::iter::once("f2".to_string()).collect(),
                ..Default::default()
            };
            app.view_mode = ViewMode::ResolveStacks;
            app
        }

        #[test]
        fn number_key_toggles_that_frame() {
            let mut app = review_app();
            let _ = app.resolve_toggle_frame(1);
            assert!(app.resolve.keepers.contains("f1"));
            let _ = app.resolve_toggle_frame(1);
            assert!(!app.resolve.keepers.contains("f1"));
        }

        #[test]
        fn out_of_range_number_is_ignored() {
            let mut app = review_app();
            let before = app.resolve.keepers.clone();
            let _ = app.resolve_toggle_frame(0);
            let _ = app.resolve_toggle_frame(9);
            assert_eq!(app.resolve.keepers, before);
        }

        #[test]
        fn reset_restores_the_auto_pick() {
            let mut app = review_app();
            let _ = app.resolve_toggle_frame(1);
            let _ = app.resolve_toggle_frame(3);
            let _ = app.resolve_reset_auto();
            let expect: HashSet<String> = std::iter::once("f2".to_string()).collect();
            assert_eq!(app.resolve.keepers, expect);
        }

        #[test]
        fn in_resolve_tracks_view_mode() {
            let mut app = review_app();
            assert!(app.in_resolve());
            app.view_mode = ViewMode::Browse;
            assert!(!app.in_resolve());
        }
    }
}
