use std::time::UNIX_EPOCH;

use iced::Task;
use isomfolio_core::indexing::thumbnail::thumbnail_cache_path;
use isomfolio_core::phash;

use super::LockUnwrap;
use super::super::{App, Msg};

impl App {
    pub(super) fn handle_stacking_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::RunStacking => self.run_stacking_task(),

            Msg::StacksUpdated => {
                self.stacking_in_flight = false;
                // When collapsed, the visible file set changes (one tile per
                // stack), so the list must reload; otherwise only the ⧉ badges do.
                if self.collapse_bursts {
                    self.load_files_task()
                } else {
                    self.load_file_side_data_task()
                }
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

            Msg::StackFlagsApplied { before, kept } => {
                if before.is_empty() {
                    return Task::none();
                }
                let total = before.len();
                self.undo_stack.push(crate::app::UndoOp::SetFlags { before });
                self.redo_stack.clear();
                self.status = if kept > 0 {
                    format!("Kept {kept}, rejected {} in stack", total - kept)
                } else {
                    format!("Rejected {total} in stack")
                };
                // Reload so the rep badge (and any expanded members) reflect the
                // new flags; the undo snapshot already covers hidden siblings.
                self.load_files_task()
            }

            _ => Task::none(),
        }
    }

    /// Cull a whole stack from its anchor frame: `keep_one` keeps the anchor as a
    /// Pick and rejects the rest; otherwise every member is rejected. The flag
    /// write (and the prior-flag snapshot for undo) runs off the UI thread.
    fn cull_stack(&mut self, anchor: String, keep_one: bool) -> Task<Msg> {
        self.context_menu = None;
        let Some(conn) = self.catalog.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                g.set_stack_flags(&anchor, keep_one).map_err(|e| e.to_string())
            },
            move |res| match res {
                Ok(before) => Msg::StackFlagsApplied {
                    before,
                    kept: if keep_one { 1 } else { 0 },
                },
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
