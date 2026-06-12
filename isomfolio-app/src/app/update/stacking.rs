use std::collections::HashMap;
use std::time::UNIX_EPOCH;

use iced::Task;
use isomfolio_core::indexing::thumbnail::thumbnail_cache_path;
use isomfolio_core::models::{AssetFile, Flag, SearchQuery};
use isomfolio_core::phash;

use super::loupe_load::decode_image_sized;
use super::LockUnwrap;
use super::super::{App, BurstCache, Msg, ResolveState, SidebarItem, StackReview, UndoOp, ViewMode};

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

            Msg::ResolveStacksLoaded { stacks, cache, threshold } => {
                if stacks.is_empty() {
                    self.status = "No stacks to review in this view".to_string();
                    return Task::none();
                }
                self.resolve = ResolveState {
                    stacks,
                    idx: 0,
                    // Open in the burst half, seeded at the catalog's stacking
                    // threshold; drag right past the seam to reach scenes.
                    tolerance: Self::burst_threshold_to_pos(threshold),
                    burst_cache: Some(cache),
                    ..Default::default()
                };
                self.view_mode = ViewMode::ResolveStacks;
                // Drop any grid selection so flag/delete keys (which target the
                // selection) are inert while reviewing — clicks pick keepers here.
                self.grid_selected.clear();
                self.enter_resolve_stack(0)
            }

            Msg::ResolveFrameLoaded { stack_idx, frame_idx, handle, dims } => {
                if matches!(self.view_mode, ViewMode::ResolveStacks) && self.resolve.idx == stack_idx {
                    self.resolve.handles.insert(frame_idx, handle);
                    self.resolve.frame_dims.insert(frame_idx, dims);
                }
                Task::none()
            }

            Msg::ToggleResolveKeeper(id) => {
                if !self.resolve.keepers.remove(&id) {
                    self.resolve.keepers.insert(id);
                }
                self.commit_keepers();
                Task::none()
            }

            Msg::ResolveSkipStack => self.advance_resolve(self.resolve.idx + 1),

            Msg::ResolvePrevStack => {
                let prev = self.resolve.idx.saturating_sub(1);
                self.enter_resolve_stack(prev)
            }

            Msg::ResolveApplyAndNext | Msg::ResolveConfirm => self.apply_resolve_and_advance(),

            Msg::ResolveResetAuto => self.resolve_reset_auto(),

            Msg::SiftSetLayout(layout) => {
                // One layout for the whole pass; persists across group navigation.
                self.resolve.layout = layout;
                Task::none()
            }

            Msg::SiftFocusFrame(i) => {
                let n = self.resolve.stacks.get(self.resolve.idx).map_or(0, |s| s.frames.len());
                if n > 0 {
                    self.resolve.focus = i.min(n - 1);
                }
                Task::none()
            }

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
    /// Loads the raw per-file phashes + sharpness and groups them **in memory**
    /// (per folder, like the persisted stacker) so the header threshold slider can
    /// later regroup live without touching the DB.
    fn open_resolve_stacks_task(&self) -> Task<Msg> {
        let Some(catalog) = self.catalog.clone() else {
            return Task::none();
        };
        let item = self.selected_item.clone();
        let mut query = self.build_search_query();
        query.collapse_bursts = false;
        query.expanded_bursts = Vec::new();
        let is_smart = self.current_album_is_smart();
        let threshold = self.app_settings.stack_threshold;
        let window_secs = self.app_settings.stack_window_secs;

        Task::perform(
            async move {
                let (files, hashes, sharpness) = {
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
                    let hashes = cat.load_phashes(&ids).unwrap_or_default();
                    let sharpness = cat.sharpness_for(&ids).unwrap_or_default();
                    (files, hashes, sharpness)
                };
                let cache = BurstCache { files, hashes, sharpness, window_secs };
                let stacks = regroup_bursts(&cache, threshold);
                (stacks, cache, threshold)
            },
            |(stacks, cache, threshold)| Msg::ResolveStacksLoaded { stacks, cache, threshold },
        )
    }

    /// Re-group the cached burst signals at the current Hamming threshold — drives
    /// the header tolerance slider for Sift Bursts. Pure of the DB; off-thread.
    pub(super) fn regroup_bursts_task(&self, seq: u64) -> Task<Msg> {
        let Some(cache) = self.resolve.burst_cache.clone() else {
            return Task::none();
        };
        let threshold = self.sift_burst_threshold();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || regroup_bursts(&cache, threshold))
                    .await
                    .unwrap_or_default()
            },
            move |stacks| Msg::SiftRegrouped { stacks, seq },
        )
    }

    /// Mirror the live keeper set into the per-group store, so navigating away and
    /// back restores it. Called after every keeper edit.
    fn commit_keepers(&mut self) {
        let idx = self.resolve.idx;
        self.resolve.decisions.insert(idx, self.resolve.keepers.clone());
    }

    /// Show stack `i`: restore its saved keeper decision (or default to the sharpest
    /// frame on first visit) and kick off a full-res decode for each frame. Exits
    /// the mode if `i` is past the end.
    pub(crate) fn enter_resolve_stack(&mut self, i: usize) -> Task<Msg> {
        let Some(stack) = self.resolve.stacks.get(i) else {
            return self.exit_resolve(true);
        };
        // Layout is a single, stable choice for the whole pass (set via the header
        // toggle) — it does NOT change per group, so navigating never flips between
        // Grid and Strip behind your back.
        self.resolve.idx = i;
        self.resolve.focus = 0;
        self.resolve.handles.clear();
        self.resolve.frame_dims.clear();
        // Restore a saved in-session decision; else seed from the frames' existing
        // Pick flags (so a keeper chosen in an earlier pass persists across exit and
        // re-entry); else fall back to the sharpest frame.
        self.resolve.keepers = self.resolve.decisions.get(&i).cloned().unwrap_or_else(|| {
            let prior: std::collections::HashSet<String> = stack
                .frames
                .iter()
                .filter(|f| f.flag == Flag::Pick)
                .map(|f| f.id.clone())
                .collect();
            if prior.is_empty() {
                std::iter::once(stack.rep_id.clone()).collect()
            } else {
                prior
            }
        });
        let tasks: Vec<Task<Msg>> = stack
            .frames
            .iter()
            .enumerate()
            .map(|(frame_idx, f)| {
                let path = f.disk_path();
                let stack_idx = i;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || decode_image_sized(&path, false))
                            .await
                            .ok()
                            .flatten()
                    },
                    move |h| match h {
                        Some((handle, dims)) => Msg::ResolveFrameLoaded { stack_idx, frame_idx, handle, dims },
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

    // The unified Sift slider is a normalized 0..1 tolerance with the seam at 0.5:
    // the left half tunes the burst Hamming threshold, the right half the scene eps.
    // The active engine — and each engine's parameter — is derived from the position.
    pub(crate) fn sift_is_scenes(&self) -> bool {
        self.resolve.tolerance >= 0.5
    }

    /// Burst Hamming threshold from the slider's left half: 0 (exact) → 16 (loose).
    pub(crate) fn sift_burst_threshold(&self) -> u32 {
        (self.resolve.tolerance.clamp(0.0, 0.5) * 32.0).round() as u32
    }

    /// Scene clustering eps from the slider's right half: 0 (tight) → 1 (loose).
    pub(crate) fn sift_scene_eps(&self) -> f32 {
        ((self.resolve.tolerance - 0.5) * 2.0).clamp(0.0, 1.0)
    }

    /// The normalized slider position for a burst threshold (the inverse of
    /// [`Self::sift_burst_threshold`]) — used to seed the slider on open.
    pub(super) fn burst_threshold_to_pos(threshold: u32) -> f32 {
        (threshold as f32 / 32.0).clamp(0.0, 0.49)
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
                self.commit_keepers();
            }
        }
        Task::none()
    }

    /// Restore the current stack's keepers to the auto-picked (sharpest) frame.
    pub(super) fn resolve_reset_auto(&mut self) -> Task<Msg> {
        if let Some(stack) = self.resolve.stacks.get(self.resolve.idx) {
            self.resolve.keepers = std::iter::once(stack.rep_id.clone()).collect();
            self.commit_keepers();
        }
        Task::none()
    }

    /// Flag the current stack (keepers → Pick, the rest → Reject), record undo,
    /// then advance. The flag write runs off-thread; when it's the *last* stack we
    /// chain the exit reload onto it so the grid reflects the final write.
    fn apply_resolve_and_advance(&mut self) -> Task<Msg> {
        if self.resolve.idx >= self.resolve.stacks.len() {
            return Task::none();
        }
        // Keeping nothing would silently reject the whole stack — almost never the
        // intent. Treat it as a skip and nudge the user, rather than blow it away.
        if self.resolve.keepers.is_empty() {
            self.status = "Nothing kept — skipped (pick at least one frame to keep)".to_string();
            return self.advance_resolve(self.resolve.idx + 1);
        }
        self.commit_keepers();
        let stack = &self.resolve.stacks[self.resolve.idx];
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

/// Group the cached view files into review stacks **in memory** at a given Hamming
/// `threshold`, per folder (matching the persisted stacker). Files keep
/// first-appearance order; only groups of ≥2 are returned, each tagged with its
/// sharpest frame as `rep_id`. Pure — drives both the initial queue and the live
/// threshold slider.
fn regroup_bursts(cache: &BurstCache, threshold: u32) -> Vec<StackReview> {
    let mut order: Vec<String> = Vec::new();
    let mut by_folder: HashMap<String, Vec<&AssetFile>> = HashMap::new();
    for f in &cache.files {
        if !cache.hashes.contains_key(&f.id) {
            continue; // no phash yet → can't stack
        }
        if !by_folder.contains_key(&f.folder) {
            order.push(f.folder.clone());
        }
        by_folder.entry(f.folder.clone()).or_default().push(f);
    }

    let mut out: Vec<StackReview> = Vec::new();
    for folder in order {
        let Some(files) = by_folder.remove(&folder) else { continue };
        let items: Vec<phash::HashedFile> = files
            .iter()
            .map(|f| phash::HashedFile {
                hash: cache.hashes.get(&f.id).copied().unwrap_or(0),
                // Match the persisted stacker: COALESCE(exif_date, modified_time).
                time: f.exif_date_unix.unwrap_or(f.mtime_unix),
            })
            .collect();
        for group in phash::group_stacks(&items, threshold, cache.window_secs) {
            let frames: Vec<AssetFile> = group.iter().map(|&i| files[i].clone()).collect();
            let sharpness: Vec<f64> =
                frames.iter().map(|f| cache.sharpness.get(&f.id).copied().unwrap_or(0.0)).collect();
            let rep_id = frames
                .iter()
                .zip(&sharpness)
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(f, _)| f.id.clone())
                .unwrap_or_default();
            out.push(StackReview { frames, sharpness, rep_id });
        }
    }
    out
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

    mod regroup_bursts_fn {
        use super::*;

        /// `n` near-identical frames (hashes within a few bits) at consecutive
        /// seconds, in folder `folder`, ids `prefix0..`.
        fn burst(cache: &mut BurstCache, folder: &str, prefix: &str, hashes: &[u64], sharp: &[f64]) {
            for (i, (&h, &s)) in hashes.iter().zip(sharp).enumerate() {
                let id = format!("{prefix}{i}");
                let mut f = file(&id);
                f.folder = folder.to_string();
                f.exif_date_unix = Some(i as i64); // 1s apart, in order
                cache.hashes.insert(id.clone(), h);
                cache.sharpness.insert(id.clone(), s);
                cache.files.push(f);
            }
        }

        fn empty_cache() -> BurstCache {
            BurstCache { files: vec![], hashes: Default::default(), sharpness: Default::default(), window_secs: 10 }
        }

        #[test]
        fn groups_near_duplicates_within_threshold() {
            let mut c = empty_cache();
            burst(&mut c, "/a", "a", &[0b000, 0b001, 0b011], &[5.0, 9.0, 1.0]);
            let stacks = regroup_bursts(&c, 4);
            assert_eq!(stacks.len(), 1);
            assert_eq!(stacks[0].frames.len(), 3);
            assert_eq!(stacks[0].rep_id, "a1"); // sharpest (9.0)
        }

        #[test]
        fn tighter_threshold_splits_the_group() {
            let mut c = empty_cache();
            // a0..a1 identical (dist 0); a2 is 3 bits away.
            burst(&mut c, "/a", "a", &[0b000, 0b000, 0b111], &[1.0, 2.0, 3.0]);
            // threshold 0 → only the two identical frames group; the 3-bit one drops.
            let tight = regroup_bursts(&c, 0);
            assert_eq!(tight.len(), 1);
            assert_eq!(tight[0].frames.len(), 2);
            // looser → all three.
            let loose = regroup_bursts(&c, 4);
            assert_eq!(loose[0].frames.len(), 3);
        }

        #[test]
        fn folders_group_independently() {
            let mut c = empty_cache();
            burst(&mut c, "/a", "a", &[0, 0], &[1.0, 2.0]);
            burst(&mut c, "/b", "b", &[0, 0], &[1.0, 2.0]);
            let stacks = regroup_bursts(&c, 2);
            assert_eq!(stacks.len(), 2, "same hashes in different folders stay separate");
        }

        #[test]
        fn files_without_a_phash_are_skipped() {
            let mut c = empty_cache();
            burst(&mut c, "/a", "a", &[0, 0], &[1.0, 2.0]);
            let mut lone = file("nohash");
            lone.folder = "/a".to_string();
            c.files.push(lone); // no entry in hashes
            let stacks = regroup_bursts(&c, 2);
            assert_eq!(stacks[0].frames.len(), 2);
            assert!(stacks[0].frames.iter().all(|f| f.id != "nohash"));
        }
    }

    mod resolve_keeper_keys {
        use super::*;
        use std::collections::HashSet;

        fn review_app() -> App {
            let mut app = App::new(None).0;
            let frames = vec![file("f1"), file("f2"), file("f3")];
            app.resolve = ResolveState {
                stacks: vec![StackReview {
                    frames,
                    sharpness: vec![1.0, 3.0, 2.0],
                    rep_id: "f2".to_string(),
                }],
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

        fn two_stack_app() -> App {
            let mut app = App::new(None).0;
            app.resolve = ResolveState {
                stacks: vec![
                    StackReview {
                        frames: vec![file("a1"), file("a2")],
                        sharpness: vec![5.0, 1.0],
                        rep_id: "a1".to_string(),
                    },
                    StackReview {
                        frames: vec![file("b1"), file("b2")],
                        sharpness: vec![2.0, 8.0],
                        rep_id: "b2".to_string(),
                    },
                ],
                idx: 0,
                keepers: std::iter::once("a1".to_string()).collect(),
                ..Default::default()
            };
            app.view_mode = ViewMode::ResolveStacks;
            app
        }

        #[test]
        fn decision_survives_navigation_away_and_back() {
            let mut app = two_stack_app();
            // Override the auto-pick on stack 0: keep a2 as well.
            let _ = app.resolve_toggle_frame(2);
            let stack0 = app.resolve.keepers.clone();
            assert!(stack0.contains("a1") && stack0.contains("a2"));
            // Move to stack 1 (its own default), then back to stack 0.
            let _ = app.enter_resolve_stack(1);
            assert_eq!(app.resolve.keepers, std::iter::once("b2".to_string()).collect());
            let _ = app.enter_resolve_stack(0);
            assert_eq!(app.resolve.keepers, stack0, "stack 0 choice must be restored");
        }

        #[test]
        fn fresh_group_defaults_to_the_sharpest() {
            let mut app = two_stack_app();
            // Never visited stack 1; entering it should default to its rep (b2).
            let _ = app.enter_resolve_stack(1);
            assert_eq!(app.resolve.keepers, std::iter::once("b2".to_string()).collect());
        }
    }

    mod sharpness_rank {
        use super::*;

        #[test]
        fn ranks_one_for_sharpest_descending() {
            let s = StackReview {
                frames: vec![file("f0"), file("f1"), file("f2")],
                sharpness: vec![1.0, 3.0, 2.0],
                rep_id: "f1".to_string(),
            };
            assert_eq!(s.sharpness_rank(0), 3);
            assert_eq!(s.sharpness_rank(1), 1);
            assert_eq!(s.sharpness_rank(2), 2);
        }

        #[test]
        fn ties_get_distinct_stable_ranks() {
            let s = StackReview {
                frames: vec![file("f0"), file("f1")],
                sharpness: vec![4.0, 4.0],
                rep_id: "f0".to_string(),
            };
            assert_eq!(s.sharpness_rank(0), 1);
            assert_eq!(s.sharpness_rank(1), 2);
        }
    }

    mod sift_layout {
        use super::*;
        use crate::app::SurfaceLayout;

        fn app_with_group(n: usize) -> App {
            let mut app = App::new(None).0;
            let frames: Vec<_> = (0..n).map(|i| file(&format!("f{i}"))).collect();
            let sharpness: Vec<f64> = (0..n).map(|i| i as f64).collect();
            app.resolve = ResolveState {
                stacks: vec![StackReview { frames, sharpness, rep_id: "f0".to_string() }],
                ..Default::default()
            };
            app.view_mode = ViewMode::ResolveStacks;
            app
        }

        #[test]
        fn set_layout_changes_layout() {
            let mut app = app_with_group(3);
            let _ = app.handle_stacking_msg(Msg::SiftSetLayout(SurfaceLayout::Strip));
            assert_eq!(app.resolve.layout, SurfaceLayout::Strip);
        }

        #[test]
        fn focus_frame_clamps_to_range() {
            let mut app = app_with_group(3);
            let _ = app.handle_stacking_msg(Msg::SiftFocusFrame(99));
            assert_eq!(app.resolve.focus, 2);
        }

        #[test]
        fn layout_is_stable_across_groups() {
            // A chosen layout must not flip when navigating between groups of
            // different sizes (the per-group auto-flip bug).
            let mut app = app_with_group(3);
            app.resolve.stacks.push(StackReview {
                frames: (0..8).map(|i| file(&format!("b{i}"))).collect(),
                sharpness: (0..8).map(|i| i as f64).collect(),
                rep_id: "b0".to_string(),
            });
            let _ = app.handle_stacking_msg(Msg::SiftSetLayout(SurfaceLayout::Strip));
            let _ = app.enter_resolve_stack(1); // big group
            assert_eq!(app.resolve.layout, SurfaceLayout::Strip);
            let _ = app.enter_resolve_stack(0); // small group — stays Strip
            assert_eq!(app.resolve.layout, SurfaceLayout::Strip);
        }

        #[test]
        fn keepers_seed_from_existing_pick_flags() {
            // A frame already flagged Pick (a keeper from a prior pass) is restored
            // as the keeper on entry, instead of resetting to the sharpest.
            let mut app = App::new(None).0;
            let mut f_keep = file("k");
            f_keep.flag = isomfolio_core::models::Flag::Pick;
            app.resolve = ResolveState {
                stacks: vec![StackReview {
                    frames: vec![file("a"), f_keep, file("c")],
                    sharpness: vec![9.0, 1.0, 5.0], // "a" is sharpest
                    rep_id: "a".to_string(),
                }],
                ..Default::default()
            };
            app.view_mode = ViewMode::ResolveStacks;
            let _ = app.enter_resolve_stack(0);
            assert!(app.resolve.keepers.contains("k"), "prior Pick restored as keeper");
            assert!(!app.resolve.keepers.contains("a"), "not reset to the sharpest");
        }

        #[test]
        fn arrows_move_frame_focus_in_strip() {
            let mut app = app_with_group(4);
            app.resolve.layout = SurfaceLayout::Strip;
            let _ = app.update(Msg::Navigate { dx: 1, dy: 0 });
            assert_eq!(app.resolve.focus, 1);
            let _ = app.update(Msg::Navigate { dx: -1, dy: 0 });
            assert_eq!(app.resolve.focus, 0);
        }

        #[test]
        fn arrows_change_group_in_grid() {
            let mut app = app_with_group(3);
            app.resolve.stacks.push(StackReview {
                frames: vec![file("g0"), file("g1")],
                sharpness: vec![1.0, 2.0],
                rep_id: "g1".to_string(),
            });
            app.resolve.layout = SurfaceLayout::Grid;
            let _ = app.update(Msg::Navigate { dx: 1, dy: 0 });
            assert_eq!(app.resolve.idx, 1);
        }
    }

    mod sift_tolerance_axis {
        use super::*;

        fn app_at(t: f32) -> App {
            let mut a = App::new(None).0;
            a.resolve.tolerance = t;
            a
        }

        #[test]
        fn seam_at_half_selects_engine() {
            assert!(!app_at(0.0).sift_is_scenes());
            assert!(!app_at(0.49).sift_is_scenes());
            assert!(app_at(0.5).sift_is_scenes());
            assert!(app_at(0.9).sift_is_scenes());
        }

        #[test]
        fn burst_half_maps_to_threshold() {
            assert_eq!(app_at(0.0).sift_burst_threshold(), 0);
            assert_eq!(app_at(0.25).sift_burst_threshold(), 8);
            assert_eq!(app_at(0.5).sift_burst_threshold(), 16);
        }

        #[test]
        fn scene_half_maps_to_eps() {
            assert!(app_at(0.5).sift_scene_eps().abs() < 1e-6);
            assert!((app_at(0.75).sift_scene_eps() - 0.5).abs() < 1e-6);
            assert!((app_at(1.0).sift_scene_eps() - 1.0).abs() < 1e-6);
        }

        #[test]
        fn open_position_round_trips_default_threshold() {
            let mut a = App::new(None).0;
            a.resolve.tolerance = App::burst_threshold_to_pos(8);
            assert_eq!(a.sift_burst_threshold(), 8);
            assert!(!a.sift_is_scenes(), "default opens in the burst half");
        }
    }
}
