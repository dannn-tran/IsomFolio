use std::time::UNIX_EPOCH;

use iced::Task;
use isomfolio_core::indexing::thumbnail::thumbnail_cache_path;
use isomfolio_core::models::SearchQuery;
use isomfolio_core::scene_embed::{self, SceneItem, SCENE_MODEL};

use super::LockUnwrap;
use super::super::{App, Msg, ResolveState, SceneCache, SceneProgress, SidebarItem, StackReview, ViewMode};

/// Files embedded per chunk between progress updates — small enough that the bar
/// advances a few times a second, large enough that per-chunk lock/dispatch
/// overhead stays negligible.
const SCENE_CHUNK: usize = 64;

impl App {
    pub(super) fn handle_scenes_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::RunSceneEmbedding => self.run_scene_embedding_task(),

            Msg::SceneEmbedStarted(needing) => {
                if needing.is_empty() {
                    self.scene_pass_starting = false;
                    return Task::none();
                }
                self.scene_pass_starting = false;
                self.scene_pass = Some(SceneProgress {
                    total: needing.len(),
                    done: 0,
                    queue: needing,
                    start_at: std::time::Instant::now(),
                });
                self.next_scene_chunk()
            }

            Msg::SceneEmbedChunkDone { processed, total_embedded } => {
                self.scene_embed_count = total_embedded;
                if let Some(p) = self.scene_pass.as_mut() {
                    p.done += processed;
                }
                self.next_scene_chunk()
            }

            Msg::SceneEmbeddingDone(total) => {
                self.scene_embed_count = total;
                Task::none()
            }

            Msg::OpenResolveScenes => self.open_resolve_scenes_task(),

            Msg::ResolveScenesLoaded { stacks, cache, eps } => {
                if stacks.is_empty() {
                    self.status = "No scenes to review in this view".to_string();
                    return Task::none();
                }
                self.resolve = ResolveState {
                    stacks,
                    idx: 0,
                    scenes: true,
                    tolerance: eps,
                    scene_cache: Some(cache),
                    ..Default::default()
                };
                self.view_mode = ViewMode::ResolveStacks;
                self.grid_selected.clear();
                self.enter_resolve_stack(0)
            }

            Msg::SiftToleranceChanged(v) => {
                self.resolve.tolerance = v;
                Task::none()
            }

            Msg::SiftRegroup => {
                // Bump the request id so any in-flight regroup's result is dropped
                // when it returns — the slider may have moved on. Mark in-flight.
                self.resolve.regroup_seq += 1;
                self.resolve.regrouping = true;
                let seq = self.resolve.regroup_seq;
                if self.resolve.scenes {
                    self.regroup_scenes_task(seq)
                } else {
                    self.regroup_bursts_task(seq)
                }
            }

            Msg::SiftRegrouped { stacks, seq } => {
                // Drop a stale result (the slider moved again since this kicked off).
                if seq != self.resolve.regroup_seq {
                    return Task::none();
                }
                self.resolve.regrouping = false;
                if stacks.is_empty() {
                    // No groups at this tolerance — keep the current queue, just nudge.
                    self.status = "No groups at this tolerance — adjust the slider".to_string();
                    return Task::none();
                }
                // Grouping changed, so prior per-group decisions no longer line up;
                // committed keep/reject already live as flags in the DB.
                self.resolve.stacks = stacks;
                self.resolve.decisions.clear();
                self.enter_resolve_stack(0)
            }

            _ => Task::none(),
        }
    }

    /// Refresh the embedded-frame count for the Settings readout without running
    /// a pass — cheap COUNT, fired on catalog open.
    pub(crate) fn load_scene_count_task(&self) -> Task<Msg> {
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                g.count_scene_embeddings(SCENE_MODEL).unwrap_or(0) as usize
            },
            Msg::SceneEmbeddingDone,
        )
    }

    /// Kick the background scene-embedding pass: compute the needing-list off the
    /// lock, then hand it to `SceneEmbedStarted` which opens the panel task and
    /// drains it a chunk at a time. Re-entrant triggers (sync + thumbnail-drain
    /// both fire this) are coalesced — a pass already running or starting is a
    /// no-op so two passes can't embed the same files at once.
    pub(crate) fn run_scene_embedding_task(&mut self) -> Task<Msg> {
        if self.scene_pass.is_some() || self.scene_pass_starting {
            return Task::none();
        }
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        self.scene_pass_starting = true;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let g = conn.lock_unwrap();
                    g.files_needing_scene_embedding(SCENE_MODEL).unwrap_or_default()
                })
                .await
                .unwrap_or_default()
            },
            Msg::SceneEmbedStarted,
        )
    }

    /// Embed the next `SCENE_CHUNK` files from the in-flight pass: decode + compute
    /// *off the lock*, write the chunk back *under the lock* (stacking's
    /// discipline). When the queue drains, close the pass and leave a ✓ toast.
    fn next_scene_chunk(&mut self) -> Task<Msg> {
        let Some(pass) = self.scene_pass.as_mut() else {
            return Task::none();
        };
        if pass.queue.is_empty() {
            let total = pass.total;
            self.scene_pass = None;
            if total > 0 {
                self.bg_mark_done("Scenes embedded", format!("{total} frames"));
            }
            return Task::none();
        }
        let take = pass.queue.len().min(SCENE_CHUNK);
        let chunk: Vec<(String, i64)> = pass.queue.drain(..take).collect();
        let Some(conn) = self.catalog.clone() else {
            self.scene_pass = None;
            return Task::none();
        };
        let catalog_dir = self.catalog_dir.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    // Advance the bar by files *attempted*, not embedded — some may
                    // lack a thumbnail and be skipped, but they still leave the queue.
                    let processed = chunk.len();
                    let rows = embed_thumbnails(&catalog_dir, chunk);
                    let total_embedded = {
                        let g = conn.lock_unwrap();
                        if !rows.is_empty() {
                            if let Err(e) = g.store_scene_embeddings(SCENE_MODEL, &rows) {
                                eprintln!("[scene] store_scene_embeddings failed: {e}");
                            }
                        }
                        g.count_scene_embeddings(SCENE_MODEL).unwrap_or(0) as usize
                    };
                    (processed, total_embedded)
                })
                .await
                .unwrap_or((0, 0))
            },
            |(processed, total_embedded)| Msg::SceneEmbedChunkDone { processed, total_embedded },
        )
    }

    /// Build the "Review Scenes" queue for the current view: ensure every visible
    /// file is embedded (compute any stragglers the background pass hasn't reached
    /// yet), then cluster the embeddings and emit groups of ≥2, sharpest-first.
    fn open_resolve_scenes_task(&self) -> Task<Msg> {
        let Some(catalog) = self.catalog.clone() else {
            return Task::none();
        };
        let catalog_dir = self.catalog_dir.clone();
        let item = self.selected_item.clone();
        let mut query = self.build_search_query();
        query.collapse_bursts = false;
        query.expanded_bursts = Vec::new();
        let is_smart = self.current_album_is_smart();
        let eps = self.app_settings.scene_eps;
        let min_pts = self.app_settings.scene_min_pts as usize;

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let files = {
                        let cat = catalog.lock_unwrap();
                        search_scope(&cat, &item, query, is_smart)
                    };
                    let ids: Vec<String> = files.iter().map(|f| f.id.clone()).collect();

                    // Catch up any unembedded files in this view so the cluster is complete.
                    let needing: Vec<(String, i64)> = {
                        let cat = catalog.lock_unwrap();
                        cat.files_needing_scene_embedding(SCENE_MODEL)
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|(id, _)| ids.contains(id))
                            .collect()
                    };
                    if !needing.is_empty() {
                        let rows = embed_thumbnails(&catalog_dir, needing);
                        if !rows.is_empty() {
                            let cat = catalog.lock_unwrap();
                            let _ = cat.store_scene_embeddings(SCENE_MODEL, &rows);
                        }
                    }

                    let (embeddings, sharpness) = {
                        let cat = catalog.lock_unwrap();
                        let emb = cat.load_scene_embeddings(SCENE_MODEL, &ids).unwrap_or_default();
                        let sharp = cat.sharpness_for(&ids).unwrap_or_default();
                        (emb, sharp)
                    };
                    // Whiten across the whole view before clustering — without it a
                    // homogeneous shoot (shared backdrop) collapses into one mega-scene.
                    let raw: Vec<Vec<f32>> = embeddings.iter().map(|(_, v)| v.clone()).collect();
                    let whitened: Vec<(String, Vec<f32>)> = embeddings
                        .iter()
                        .map(|(id, _)| id.clone())
                        .zip(scene_embed::whiten(&raw))
                        .collect();
                    let stacks =
                        build_scene_review(files.clone(), whitened.clone(), sharpness.clone(), eps, min_pts);
                    // Keep the whitened inputs so the header slider can re-cluster
                    // live (in memory) without re-querying or re-embedding.
                    let cache = SceneCache { files, whitened, sharpness, min_pts };
                    (stacks, cache, eps)
                })
                .await
                .unwrap_or_else(|_| (Vec::new(), SceneCache::default(), 0.0))
            },
            |(stacks, cache, eps)| Msg::ResolveScenesLoaded { stacks, cache, eps },
        )
    }

    /// Re-cluster the cached scene signals at the current tolerance — drives the
    /// header looseness slider. Pure of the DB; runs off-thread since DBSCAN over a
    /// large view is O(n²).
    fn regroup_scenes_task(&self, seq: u64) -> Task<Msg> {
        let Some(cache) = self.resolve.scene_cache.clone() else {
            return Task::none();
        };
        let eps = self.resolve.tolerance;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    build_scene_review(cache.files, cache.whitened, cache.sharpness, eps, cache.min_pts)
                })
                .await
                .unwrap_or_default()
            },
            move |stacks| Msg::SiftRegrouped { stacks, seq },
        )
    }
}

/// Decode each `(file_id, mtime)`'s cached thumbnail and compute its scene
/// embedding. Files without a thumbnail yet are skipped (re-tried next pass).
/// Pure of any DB lock — caller holds none while this runs.
fn embed_thumbnails(catalog_dir: &str, needing: Vec<(String, i64)>) -> Vec<(String, i64, Vec<f32>)> {
    let mut rows = Vec::new();
    for (id, mtime) in needing {
        let path = thumbnail_cache_path(catalog_dir, &id);
        let p = std::path::Path::new(&path);
        if !p.exists() {
            continue;
        }
        if let Ok(img) = image::open(p) {
            let _ = UNIX_EPOCH; // mtime is the file's modified_time, matched on read
            rows.push((id, mtime, scene_embed::scene_embedding(&img)));
        }
    }
    rows
}

fn search_scope(
    cat: &isomfolio_core::Catalog,
    item: &SidebarItem,
    query: SearchQuery,
    is_smart: bool,
) -> Vec<isomfolio_core::models::AssetFile> {
    match item {
        SidebarItem::AllFiles => cat.search(&query).unwrap_or_default(),
        SidebarItem::Folder(path) => {
            let q = SearchQuery { folder_path: Some(path.clone()), folder_recursive: true, ..query };
            cat.search(&q).unwrap_or_default()
        }
        SidebarItem::Album(album_id) => {
            if is_smart {
                cat.search(&query).unwrap_or_default()
            } else {
                cat.search_manual_album(album_id, &query).unwrap_or_default()
            }
        }
        SidebarItem::Import(batch_id) => {
            let q = SearchQuery { import_batch: Some(*batch_id), ..query };
            cat.search(&q).unwrap_or_default()
        }
        SidebarItem::Deleted => Vec::new(),
    }
}

/// Cluster the view's embeddings into scene groups, mapping each cluster back to
/// its `AssetFile`s (sharpest first as the default keeper). Files lacking an
/// embedding are dropped. Pure — unit-tested below.
fn build_scene_review(
    files: Vec<isomfolio_core::models::AssetFile>,
    embeddings: Vec<(String, Vec<f32>)>,
    sharpness: std::collections::HashMap<String, f64>,
    eps: f32,
    min_pts: usize,
) -> Vec<StackReview> {
    use std::collections::HashMap;
    let emb: HashMap<String, Vec<f32>> = embeddings.into_iter().collect();

    // Keep only files that have an embedding, preserving the view's order.
    let kept: Vec<isomfolio_core::models::AssetFile> =
        files.into_iter().filter(|f| emb.contains_key(&f.id)).collect();
    let items: Vec<SceneItem> = kept
        .iter()
        .map(|f| SceneItem {
            embedding: emb.get(&f.id).cloned().unwrap_or_default(),
            sharpness: sharpness.get(&f.id).copied().unwrap_or(0.0),
        })
        .collect();

    scene_embed::group_scenes(&items, eps, min_pts)
        .into_iter()
        .map(|group| {
            // group is sharpest-first; rep is its head.
            let frames: Vec<_> = group.iter().map(|&i| kept[i].clone()).collect();
            let frame_sharpness: Vec<f64> =
                frames.iter().map(|f| sharpness.get(&f.id).copied().unwrap_or(0.0)).collect();
            let rep_id = frames.first().map(|f| f.id.clone()).unwrap_or_default();
            StackReview { frames, sharpness: frame_sharpness, rep_id }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use isomfolio_core::models::{AssetFile, Flag};
    use std::collections::HashMap;

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

    fn unit(v: [f32; 4]) -> Vec<f32> {
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / n).collect()
    }

    mod build_scene_review_fn {
        use super::*;

        #[test]
        fn clusters_view_into_scenes_keeper_first() {
            let files = vec![file("a1"), file("a2"), file("a3"), file("solo"), file("b1"), file("b2"), file("b3")];
            let embeddings = vec![
                ("a1".into(), unit([1.0, 0.05, 0.0, 0.0])),
                ("a2".into(), unit([1.0, 0.0, 0.05, 0.0])),
                ("a3".into(), unit([1.0, 0.03, 0.03, 0.0])),
                ("solo".into(), unit([0.0, 0.0, 1.0, 0.0])),
                ("b1".into(), unit([0.05, 1.0, 0.0, 0.0])),
                ("b2".into(), unit([0.0, 1.0, 0.05, 0.0])),
                ("b3".into(), unit([0.03, 1.0, 0.03, 0.0])),
            ];
            let sharp: HashMap<String, f64> =
                [("a1", 0.3), ("a2", 0.9), ("a3", 0.5), ("b1", 0.2), ("b2", 0.4), ("b3", 0.8)]
                    .iter()
                    .map(|(k, v)| (k.to_string(), *v))
                    .collect();

            let scenes = build_scene_review(files, embeddings, sharp, 0.1, 2);
            assert_eq!(scenes.len(), 2, "two scenes; solo dropped");
            // Sharpest frame leads each scene (a2 in A, b3 in B).
            let reps: Vec<&str> = scenes.iter().map(|s| s.rep_id.as_str()).collect();
            assert!(reps.contains(&"a2"));
            assert!(reps.contains(&"b3"));
            // solo never appears.
            assert!(scenes.iter().all(|s| s.frames.iter().all(|f| f.id != "solo")));
        }

        #[test]
        fn files_without_embeddings_are_dropped() {
            let files = vec![file("a1"), file("a2"), file("noemb")];
            let embeddings = vec![
                ("a1".into(), unit([1.0, 0.05, 0.0, 0.0])),
                ("a2".into(), unit([1.0, 0.0, 0.05, 0.0])),
            ];
            // min_pts=1 (the default): a two-frame scene forms.
            let scenes = build_scene_review(files, embeddings, HashMap::new(), 0.1, 1);
            assert_eq!(scenes.len(), 1);
            assert_eq!(scenes[0].frames.len(), 2);
        }
    }

    mod live_tolerance {
        use super::file;
        use crate::app::{App, Msg, ResolveState, StackReview, ViewMode};

        fn sr(id: &str) -> StackReview {
            StackReview {
                frames: vec![file(id), file(&format!("{id}b"))],
                sharpness: vec![2.0, 1.0],
                rep_id: id.to_string(),
            }
        }

        fn scenes_app(stacks: Vec<StackReview>) -> App {
            let mut app = App::new(None).0;
            app.resolve = ResolveState {
                stacks,
                idx: 0,
                scenes: true,
                tolerance: 0.5,
                ..Default::default()
            };
            app.view_mode = ViewMode::ResolveStacks;
            app
        }

        #[test]
        fn tolerance_change_updates_value_without_regrouping() {
            let mut app = scenes_app(vec![sr("a")]);
            let _ = app.update(Msg::SiftToleranceChanged(0.8));
            assert!((app.resolve.tolerance - 0.8).abs() < 1e-6);
            assert_eq!(app.resolve.stacks.len(), 1);
        }

        #[test]
        fn regroup_replaces_queue_and_clears_decisions() {
            let mut app = scenes_app(vec![sr("a"), sr("b")]);
            app.resolve.regroup_seq = 1;
            app.resolve.decisions.insert(0, std::iter::once("a".to_string()).collect());
            let _ = app.update(Msg::SiftRegrouped { stacks: vec![sr("x")], seq: 1 });
            assert_eq!(app.resolve.stacks.len(), 1);
            assert!(app.resolve.decisions.is_empty());
            assert_eq!(app.resolve.idx, 0);
        }

        #[test]
        fn empty_regroup_keeps_current_queue() {
            let mut app = scenes_app(vec![sr("a"), sr("b")]);
            app.resolve.regroup_seq = 1;
            let _ = app.update(Msg::SiftRegrouped { stacks: vec![], seq: 1 });
            assert_eq!(app.resolve.stacks.len(), 2);
        }

        #[test]
        fn stale_regroup_result_is_dropped() {
            let mut app = scenes_app(vec![sr("a"), sr("b")]);
            app.resolve.regroup_seq = 2; // slider moved on since seq 1 kicked off
            let _ = app.update(Msg::SiftRegrouped { stacks: vec![sr("x")], seq: 1 });
            assert_eq!(app.resolve.stacks.len(), 2, "stale result must not replace the queue");
        }
    }
}
