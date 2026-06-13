use iced::Task;
use isomfolio_core::models::Flag;

use super::LockUnwrap;
use super::super::{App, Msg, UndoOp};

/// Group `(id, value)` pairs by distinct value so a heterogeneous undo snapshot
/// (different ratings/flags/labels per file) collapses into one bulk DB write per
/// value — and a uniform forward edit stays a single statement. O(n·k), k = the
/// few distinct values; avoids requiring `Hash` on `Flag`.
fn group_by_value<V: PartialEq + Clone>(vals: &[(String, V)]) -> Vec<(V, Vec<String>)> {
    let mut groups: Vec<(V, Vec<String>)> = Vec::new();
    for (id, v) in vals {
        if let Some(g) = groups.iter_mut().find(|(gv, _)| gv == v) {
            g.1.push(id.clone());
        } else {
            groups.push((v.clone(), vec![id.clone()]));
        }
    }
    groups
}

impl App {
    /// In loupe with auto-advance on, sequence a forward `Navigate` after a cull
    /// write so flag, rating, and label all share one "verdict → next" behaviour.
    /// The DB write and the navigate are batched; outside loupe (grid multi-edit)
    /// the cull stays put.
    fn advance_after_cull(&self, db_task: Task<Msg>) -> Task<Msg> {
        if matches!(self.view_mode, super::super::ViewMode::Loupe)
            && self.app_settings.auto_advance_on_cull
        {
            Task::batch([db_task, Task::done(Msg::Navigate { dx: 1, dy: 0 })])
        } else {
            db_task
        }
    }

    pub(super) fn handle_detail_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::ToggleDetail => {
                self.detail.show = !self.detail.show;
                if self.detail.show {
                    self.detail.file_id = None;
                    self.maybe_load_detail()
                } else {
                    Task::none()
                }
            }

            Msg::FocusTagInput => {
                let focus =
                    iced::widget::operation::focus(crate::app::input_ids::detail_tag());
                let has_target =
                    self.detail_file().is_some() || !self.detail.batch_file_ids.is_empty();
                if has_target {
                    // The tag field renders whenever there's a target and the data
                    // is already loaded — open the panel and focus this same frame.
                    self.detail.show = true;
                    return focus;
                }
                if self.grid_selected.is_empty() {
                    self.status = "Select a photo to tag".to_string();
                    return Task::none();
                }
                // Cold open: load the selection first; `DetailLoaded` focuses the
                // field once it mounts.
                self.detail.show = true;
                self.pending_focus_tag = true;
                self.maybe_load_detail()
            }

            Msg::DetailLoaded {
                file_id,
                tags,
                rating,
                label,
                title,
                description,
                creator,
                rights,
                exif_tech,
            } => {
                self.detail.file_id = Some(file_id);
                self.detail.batch_file_ids.clear();
                self.detail.tags = tags;
                self.detail.rating = rating;
                self.detail.label = label;
                self.detail.title = title.clone();
                self.detail.exif_tech = exif_tech;
                self.detail.title_input = title.unwrap_or_default();
                self.detail.caption_input = description.unwrap_or_default();
                self.detail.creator_input = creator.unwrap_or_default();
                self.detail.rights_input = rights.unwrap_or_default();
                let load = self.load_all_tags_task();
                if std::mem::take(&mut self.pending_focus_tag) {
                    Task::batch([
                        load,
                        iced::widget::operation::focus(crate::app::input_ids::detail_tag()),
                    ])
                } else {
                    load
                }
            }

            Msg::BatchDetailLoaded { file_ids, tags } => {
                self.detail.file_id = None;
                self.detail.batch_file_ids = file_ids;
                self.detail.tags = tags;
                self.detail.rating = None;
                self.detail.label = None;
                self.detail.title = None;
                self.detail.exif_tech = None;
                // Batch: leave descriptive fields blank; typing applies to all.
                self.detail.title_input.clear();
                self.detail.caption_input.clear();
                self.detail.creator_input.clear();
                self.detail.rights_input.clear();
                self.load_all_tags_task()
            }

            Msg::DetailFieldChanged(field, value) => {
                use crate::app::DetailField;
                match field {
                    DetailField::Title => self.detail.title_input = value,
                    DetailField::Caption => self.detail.caption_input = value,
                    DetailField::Creator => self.detail.creator_input = value,
                    DetailField::Rights => self.detail.rights_input = value,
                }
                Task::none()
            }

            Msg::SaveDetailField(field) => {
                use crate::app::DetailField;
                // Apply to the single file or the whole batch selection.
                let ids: Vec<String> = if self.detail.batch_file_ids.is_empty() {
                    self.detail.file_id.iter().cloned().collect()
                } else {
                    self.detail.batch_file_ids.clone()
                };
                if ids.is_empty() {
                    return Task::none();
                }
                let raw = match field {
                    DetailField::Title => self.detail.title_input.clone(),
                    DetailField::Caption => self.detail.caption_input.clone(),
                    DetailField::Creator => self.detail.creator_input.clone(),
                    DetailField::Rights => self.detail.rights_input.clone(),
                };
                let value = raw.trim().to_string();
                let opt = if value.is_empty() { None } else { Some(value) };
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        let v = opt.as_deref();
                        let r = match field {
                            DetailField::Title => g.set_files_title(&ids, v),
                            DetailField::Caption => g.set_files_description(&ids, v),
                            DetailField::Creator => g.set_files_creator(&ids, v),
                            DetailField::Rights => g.set_files_rights(&ids, v),
                        };
                        r.err().map(|e| e.to_string())
                    },
                    |e| match e { Some(err) => Msg::DbError(err), None => Msg::NoOp },
                )
            }

            Msg::BatchTagsChanged => self.load_all_tags_task(),

            Msg::DetailTagInputChanged(s) => {
                self.detail.tag_input = s;
                Task::none()
            }

            Msg::AddDetailTag => {
                let tag = self.detail.tag_input.trim().to_string();
                self.detail.tag_input.clear();
                if tag.is_empty() || self.detail.tags.contains(&tag) {
                    return Task::none();
                }
                self.detail.tags.push(tag.clone());
                self.detail.push_recent_tag(&tag);
                let file_ids = self.current_detail_file_ids();
                self.record_tag_edit(true, file_ids, tag.clone());
                if !self.detail.batch_file_ids.is_empty() {
                    self.batch_add_tag_task(tag)
                } else {
                    self.save_detail_tags_task()
                }
            }

            Msg::AddDetailTagDirect(tag) => {
                let tag = tag.trim().to_string();
                if tag.is_empty() || self.detail.tags.contains(&tag) {
                    return Task::none();
                }
                self.detail.tags.push(tag.clone());
                self.detail.tag_input.clear();
                self.detail.push_recent_tag(&tag);
                let file_ids = self.current_detail_file_ids();
                self.record_tag_edit(true, file_ids, tag.clone());
                if !self.detail.batch_file_ids.is_empty() {
                    self.batch_add_tag_task(tag)
                } else {
                    self.save_detail_tags_task()
                }
            }

            Msg::RemoveDetailTag(tag) => {
                self.detail.tags.retain(|t| t != &tag);
                let file_ids = self.current_detail_file_ids();
                self.record_tag_edit(false, file_ids, tag.clone());
                if !self.detail.batch_file_ids.is_empty() {
                    self.batch_remove_tag_task(tag)
                } else {
                    self.save_detail_tags_task()
                }
            }

            Msg::AllTagsLoaded(tags) => {
                self.detail.all_tags = tags;
                Task::none()
            }

            Msg::TagsSavedResult(tags, err) => {
                self.detail.all_tags = tags;
                if let Some(e) = err {
                    self.status = format!("Error saving tags: {e}");
                }
                Task::none()
            }

            Msg::RepeatLastTag => {
                let Some(tag) = self.detail.recent_tags.first().cloned() else {
                    return Task::none();
                };
                self.handle_detail_msg(Msg::AddDetailTagDirect(tag))
            }

            Msg::SetDetailRating(n) => {
                let Some(fid) = self.detail.file_id.clone() else { return Task::none() };
                let new_rating = if self.detail.rating == Some(n) { None } else { Some(n) };
                // Same gesture as the grid/keyboard star: route through the shared
                // chokepoint so it stays in sync and undoable (no per-handler push).
                self.edit_ratings(vec![fid], new_rating)
            }

            Msg::SetFlag(flag) => {
                let ids = self.selection_target_ids();
                let db_task = self.edit_flags(ids, flag);
                self.advance_after_cull(db_task)
            }

            Msg::FlagsApplied => {
                if self.filters.flags.is_active() {
                    self.load_files_task()
                } else {
                    Task::none()
                }
            }

            Msg::SetRating(rating) => {
                let ids = self.selection_target_ids();
                let db_task = self.edit_ratings(ids, rating);
                self.advance_after_cull(db_task)
            }

            Msg::RatingsApplied => {
                if self.filters.rating.is_active() { self.load_files_task() } else { Task::none() }
            }

            Msg::FileSideDataLoaded { ratings, labels } => {
                self.file_ratings = ratings;
                self.file_labels = labels;
                Task::none()
            }

            Msg::SetColorLabel(color) => {
                let ids = self.selection_target_ids();
                if ids.is_empty() {
                    self.status = "Select photos first".to_string();
                    return Task::none();
                }
                // Pressing the same colour again clears it (toggle off). This
                // decision reads current labels, so it stays in the handler; the
                // resolved value then goes through the shared chokepoint.
                let effective = match &color {
                    Some(c) if ids.iter().all(|id| self.file_labels.get(id) == Some(c)) => None,
                    other => other.clone(),
                };
                // Reload (if the colour filter is active) is sequenced in the
                // `LabelsApplied` callback — never batched concurrently with the
                // write, which would race the re-query against the label commit.
                let db_task = self.edit_labels(ids, effective);
                self.advance_after_cull(db_task)
            }

            Msg::LabelsApplied => {
                if self.filters.color.is_some() { self.load_files_task() } else { Task::none() }
            }

            Msg::ToggleHideRejects => {
                // Convenience for the common cull: toggle between "show picks +
                // unflagged" and "show all".
                use isomfolio_core::models::FlagSelection;
                let hide = FlagSelection { pick: true, unflagged: true, reject: false };
                self.filters.flags = if self.filters.flags == hide {
                    FlagSelection::default()
                } else {
                    hide
                };
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::ToggleFlagFilter(flag) => {
                self.filters.flags = self.filters.flags.toggled(flag);
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::SetRatingFilter(rating) => {
                self.filters.rating = rating;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::SetRatingCmp(cmp) => {
                self.filters.rating_cmp = cmp;
                // Re-apply the comparator to the current star count, if one is set.
                use isomfolio_core::models::RatingFilter;
                let n = match self.filters.rating {
                    RatingFilter::AtLeast(n) | RatingFilter::Exactly(n) | RatingFilter::AtMost(n) => Some(n),
                    _ => None,
                };
                if let Some(n) = n {
                    self.filters.rating = cmp.apply(n);
                    self.mark_smart_dirty();
                    return self.load_files_task();
                }
                Task::none()
            }

            Msg::SetLocationFilter(val) => {
                self.filters.has_location = val;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::Undo => self.apply_undo_op(true),
            Msg::Redo => self.apply_undo_op(false),

            Msg::UndoApplied => {
                // Sidebar too: delete and album-membership undos change Deleted /
                // album counts.
                let t1 = self.load_files_task();
                let t2 = self.maybe_load_detail();
                let t3 = self.load_file_side_data_task();
                let t4 = self.load_sidebar_task();
                Task::batch([t1, t2, t3, t4])
            }

            other => {
                debug_assert!(false, "handle_detail_msg received misrouted message: {other:?}");
                Task::none()
            }
        }
    }

    fn current_detail_file_ids(&self) -> Vec<String> {
        if !self.detail.batch_file_ids.is_empty() {
            self.detail.batch_file_ids.clone()
        } else if let Some(ref fid) = self.detail.file_id {
            vec![fid.clone()]
        } else {
            self.grid_selected.iter().cloned().collect()
        }
    }

    fn apply_undo_op(&mut self, is_undo: bool) -> Task<Msg> {
        let op = if is_undo { self.undo_stack.pop() } else { self.redo_stack.pop() };
        let Some(op) = op else { return Task::none() };

        // Re-centre the view on the edited photos after the reload (loupe returns to
        // the image, grid re-selects it) — so undoing an edit that auto-advanced in
        // loupe puts you back where you were.
        let focus = op.edited_ids();
        self.pending_focus_files = (!focus.is_empty()).then_some(focus);

        // Self-contained ops: undo writes `before`, redo writes `after`. The op
        // round-trips between the stacks unchanged — no inverse to recompute.
        let task = match &op {
            UndoOp::Ratings { before, after } => {
                let vals = if is_undo { before } else { after };
                self.apply_ratings_mem(vals);
                self.ratings_db_task(vals.clone(), || Msg::UndoApplied)
            }
            UndoOp::Flags { before, after } => {
                let vals = if is_undo { before } else { after };
                self.apply_flags_mem(vals);
                self.flags_db_task(vals.clone(), || Msg::UndoApplied)
            }
            UndoOp::Labels { before, after } => {
                let vals = if is_undo { before } else { after };
                self.apply_labels_mem(vals);
                self.labels_db_task(vals.clone(), || Msg::UndoApplied)
            }
            UndoOp::Tag { add, file_ids, tag } => {
                // Undo applies the negation of the forward direction.
                let effective_add = if is_undo { !add } else { *add };
                self.tag_db_task(file_ids.clone(), tag.clone(), effective_add)
            }
            UndoOp::SetDeleted { ids, deleted } => {
                let effective = if is_undo { !deleted } else { *deleted };
                self.set_deleted_db_task(ids.clone(), effective)
            }
            UndoOp::Album { add, album_id, file_ids } => {
                let effective_add = if is_undo { !add } else { *add };
                self.album_membership_db_task(album_id.clone(), file_ids.clone(), effective_add)
            }
        };
        if is_undo {
            self.redo_stack.push(op);
        } else {
            self.undo_stack.push(op);
        }
        task
    }

    fn set_deleted_db_task(&self, ids: Vec<String>, deleted: bool) -> Task<Msg> {
        let Some(conn) = self.catalog.clone() else { return Task::none() };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    conn.lock_unwrap().set_files_deleted(&ids, deleted).err().map(|e| e.to_string())
                })
                .await
                .ok()
                .flatten()
            },
            |e| e.map_or(Msg::UndoApplied, Msg::DbError),
        )
    }

    fn album_membership_db_task(&self, album_id: String, file_ids: Vec<String>, add: bool) -> Task<Msg> {
        let Some(conn) = self.catalog.clone() else { return Task::none() };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                for fid in &file_ids {
                    let r = if add {
                        g.add_file_to_album(&album_id, fid)
                    } else {
                        g.remove_file_from_album(&album_id, fid)
                    };
                    if let Err(e) = r {
                        return Some(e.to_string());
                    }
                }
                None
            },
            |e| e.map_or(Msg::UndoApplied, Msg::DbError),
        )
    }

    // Editing chokepoints — every reversible edit flows through one of these, so
    // recording an undo step is a property of the write path, not something each
    // handler has to remember (the gap that left the detail-panel star un-undoable).

    /// Snapshot, push the op, clear the redo branch. Bounded so a long session
    /// can't grow the history without limit.
    pub(super) fn push_undo(&mut self, op: UndoOp) {
        const CAP: usize = 200;
        self.undo_stack.push(op);
        self.redo_stack.clear();
        let len = self.undo_stack.len();
        if len > CAP {
            self.undo_stack.drain(0..len - CAP);
        }
    }

    pub(super) fn edit_ratings(&mut self, ids: Vec<String>, new: Option<i32>) -> Task<Msg> {
        if ids.is_empty() {
            self.status = "Select photos first".to_string();
            return Task::none();
        }
        let before: Vec<(String, Option<i32>)> =
            ids.iter().map(|id| (id.clone(), self.file_ratings.get(id).copied())).collect();
        let after: Vec<(String, Option<i32>)> = ids.iter().map(|id| (id.clone(), new)).collect();
        self.apply_ratings_mem(&after);
        if ids.len() == 1 && self.detail.file_id.as_deref() == Some(ids[0].as_str()) {
            self.detail.rating = new;
        }
        self.push_undo(UndoOp::Ratings { before, after: after.clone() });
        self.ratings_db_task(after, || Msg::RatingsApplied)
    }

    pub(super) fn edit_flags(&mut self, ids: Vec<String>, flag: Flag) -> Task<Msg> {
        if ids.is_empty() {
            self.status = "Select photos first".to_string();
            return Task::none();
        }
        let before: Vec<(String, Flag)> = ids
            .iter()
            .map(|id| {
                let f = self.files.iter().find(|f| &f.id == id).map(|f| f.flag).unwrap_or(Flag::Unflagged);
                (id.clone(), f)
            })
            .collect();
        let after: Vec<(String, Flag)> = ids.iter().map(|id| (id.clone(), flag)).collect();
        self.apply_flags_mem(&after);
        self.push_undo(UndoOp::Flags { before, after: after.clone() });
        self.flags_db_task(after, || Msg::FlagsApplied)
    }

    pub(super) fn edit_labels(&mut self, ids: Vec<String>, label: Option<String>) -> Task<Msg> {
        if ids.is_empty() {
            self.status = "Select photos first".to_string();
            return Task::none();
        }
        let before: Vec<(String, Option<String>)> =
            ids.iter().map(|id| (id.clone(), self.file_labels.get(id).cloned())).collect();
        let after: Vec<(String, Option<String>)> =
            ids.iter().map(|id| (id.clone(), label.clone())).collect();
        self.apply_labels_mem(&after);
        self.push_undo(UndoOp::Labels { before, after: after.clone() });
        self.labels_db_task(after, || Msg::LabelsApplied)
    }

    /// Tags don't carry per-file value snapshots (they're a toggle of one tag over
    /// a set); the forward DB write + in-memory `detail.tags` upkeep stay in the
    /// tag handlers, this just registers the reversible step.
    pub(super) fn record_tag_edit(&mut self, add: bool, file_ids: Vec<String>, tag: String) {
        self.push_undo(UndoOp::Tag { add, file_ids, tag });
    }

    fn apply_ratings_mem(&mut self, vals: &[(String, Option<i32>)]) {
        for (id, r) in vals {
            match r {
                Some(n) if *n > 0 => { self.file_ratings.insert(id.clone(), *n); }
                _ => { self.file_ratings.remove(id); }
            }
        }
    }

    fn apply_flags_mem(&mut self, vals: &[(String, Flag)]) {
        for (id, flag) in vals {
            if let Some(f) = self.files.iter_mut().find(|f| &f.id == id) {
                f.flag = *flag;
            }
        }
    }

    fn apply_labels_mem(&mut self, vals: &[(String, Option<String>)]) {
        for (id, label) in vals {
            match label {
                Some(c) => { self.file_labels.insert(id.clone(), c.clone()); }
                None => { self.file_labels.remove(id); }
            }
        }
    }

    fn ratings_db_task(&self, vals: Vec<(String, Option<i32>)>, done: fn() -> Msg) -> Task<Msg> {
        let Some(conn) = self.catalog.clone() else { return Task::none() };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                for (val, ids) in group_by_value(&vals) {
                    if let Err(e) = g.set_files_rating(&ids, val) {
                        return Some(e.to_string());
                    }
                }
                None
            },
            move |e| e.map_or_else(done, Msg::DbError),
        )
    }

    fn flags_db_task(&self, vals: Vec<(String, Flag)>, done: fn() -> Msg) -> Task<Msg> {
        let Some(conn) = self.catalog.clone() else { return Task::none() };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                for (val, ids) in group_by_value(&vals) {
                    if let Err(e) = g.set_files_flag(&ids, val) {
                        return Some(e.to_string());
                    }
                }
                None
            },
            move |e| e.map_or_else(done, Msg::DbError),
        )
    }

    fn labels_db_task(&self, vals: Vec<(String, Option<String>)>, done: fn() -> Msg) -> Task<Msg> {
        let Some(conn) = self.catalog.clone() else { return Task::none() };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                for (val, ids) in group_by_value(&vals) {
                    if let Err(e) = g.set_files_label(&ids, val.as_deref()) {
                        return Some(e.to_string());
                    }
                }
                None
            },
            move |e| e.map_or_else(done, Msg::DbError),
        )
    }

    fn tag_db_task(&self, file_ids: Vec<String>, tag: String, add: bool) -> Task<Msg> {
        let Some(conn) = self.catalog.clone() else { return Task::none() };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                let r = if add {
                    g.add_tag_to_files(&file_ids, &tag)
                } else {
                    g.remove_tag_from_files(&file_ids, &tag)
                };
                r.err().map(|e| e.to_string())
            },
            |e| e.map_or(Msg::UndoApplied, Msg::DbError),
        )
    }

    pub(super) fn save_detail_tags_task(&self) -> Task<Msg> {
        let Some(ref fid) = self.detail.file_id else { return Task::none() };
        let fid = fid.clone();
        let tags = self.detail.tags.clone();
        let Some(conn) = self.catalog.clone() else { return Task::none() };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                let err = g.upsert_tags(&fid, &tags).err().map(|e| e.to_string());
                let all_tags = g
                    .get_all_tags()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(t, _)| t)
                    .collect::<Vec<_>>();
                (all_tags, err)
            },
            |(tags, err)| Msg::TagsSavedResult(tags, err),
        )
    }

    fn batch_add_tag_task(&self, tag: String) -> Task<Msg> {
        let file_ids = self.detail.batch_file_ids.clone();
        let Some(conn) = self.catalog.clone() else { return Task::none() };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                g.add_tag_to_files(&file_ids, &tag).err().map(|e| e.to_string())
            },
            |e| e.map_or(Msg::BatchTagsChanged, Msg::DbError),
        )
    }

    fn batch_remove_tag_task(&self, tag: String) -> Task<Msg> {
        let file_ids = self.detail.batch_file_ids.clone();
        let Some(conn) = self.catalog.clone() else { return Task::none() };
        Task::perform(
            async move {
                let g = conn.lock_unwrap();
                g.remove_tag_from_files(&file_ids, &tag).err().map(|e| e.to_string())
            },
            |e| e.map_or(Msg::BatchTagsChanged, Msg::DbError),
        )
    }
}

#[cfg(test)]
mod undo_redo {
    use super::*;
    use isomfolio_core::catalog::Catalog;
    use isomfolio_core::models::{AssetFile, Flag};
    use std::sync::{Arc, Mutex};

    // apply_undo_op short-circuits when no catalog is attached (it pushes the
    // inverse and mutates in-memory state only after that guard), so a real
    // in-memory catalog is required even though the async DB write never runs
    // in a unit test. These tests assert the in-memory + stack-symmetry meat.
    fn app_with_catalog() -> App {
        let mut a = App::new(None).0;
        let cat = Catalog::open(":memory:").expect("open in-memory catalog");
        a.catalog = Some(Arc::new(Mutex::new(cat)));
        a
    }

    fn file(id: &str, flag: Flag) -> AssetFile {
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
            flag,
            exif_date_unix: Some(0),
            gps_lat: None,
            gps_lon: None,
        }
    }

    #[test]
    fn rating_undo_restores_then_redo_reapplies() {
        let mut a = app_with_catalog();
        a.file_ratings.insert("f1".into(), 4); // current state after the edit
        a.undo_stack.push(UndoOp::Ratings {
            before: vec![("f1".into(), None)],
            after: vec![("f1".into(), Some(4))],
        });

        let _ = a.apply_undo_op(true);
        assert_eq!(a.file_ratings.get("f1"), None, "undo writes the `before` side");
        assert_eq!(a.undo_stack.len(), 0);
        assert_eq!(a.redo_stack.len(), 1, "op moved to the redo stack unchanged");

        let _ = a.apply_undo_op(false);
        assert_eq!(a.file_ratings.get("f1"), Some(&4), "redo writes the `after` side");
        assert_eq!(a.redo_stack.len(), 0);
        assert_eq!(a.undo_stack.len(), 1, "op moved back to the undo stack");
    }

    #[test]
    fn flag_undo_restores_then_redo_reapplies() {
        let mut a = app_with_catalog();
        a.files = vec![file("f1", Flag::Pick)];
        a.undo_stack.push(UndoOp::Flags {
            before: vec![("f1".into(), Flag::Unflagged)],
            after: vec![("f1".into(), Flag::Pick)],
        });

        let _ = a.apply_undo_op(true);
        assert_eq!(a.files[0].flag, Flag::Unflagged, "undo restores the prior flag");

        let _ = a.apply_undo_op(false);
        assert_eq!(a.files[0].flag, Flag::Pick, "redo re-applies the flag");
    }

    #[test]
    fn label_undo_restores_then_redo_reapplies() {
        let mut a = app_with_catalog();
        a.file_labels.insert("f1".into(), "red".into());
        a.undo_stack.push(UndoOp::Labels {
            before: vec![("f1".into(), None)],
            after: vec![("f1".into(), Some("red".into()))],
        });

        let _ = a.apply_undo_op(true);
        assert_eq!(a.file_labels.get("f1"), None, "undo clears the label");

        let _ = a.apply_undo_op(false);
        assert_eq!(a.file_labels.get("f1").map(String::as_str), Some("red"), "redo re-applies");
    }

    #[test]
    fn tag_op_round_trips_unchanged_across_stacks() {
        let mut a = app_with_catalog();
        a.undo_stack.push(UndoOp::Tag { add: true, file_ids: vec!["f1".into()], tag: "beach".into() });

        let _ = a.apply_undo_op(true);
        assert_eq!(a.undo_stack.len(), 0);
        assert!(
            matches!(a.redo_stack.last(), Some(UndoOp::Tag { add: true, tag, .. }) if tag == "beach"),
            "self-contained: the op moves to redo unchanged (direction flips at apply time)",
        );

        let _ = a.apply_undo_op(false);
        assert!(matches!(a.undo_stack.last(), Some(UndoOp::Tag { add: true, .. })));
        assert_eq!(a.redo_stack.len(), 0);
    }

    // The whole point of the chokepoint: editing records an undo step without the
    // handler doing anything — the class of bug that left the detail star un-undoable.
    #[test]
    fn edit_chokepoint_records_and_round_trips() {
        let mut a = app_with_catalog();
        a.files = vec![file("f1", Flag::Unflagged)];

        let _ = a.edit_flags(vec!["f1".into()], Flag::Reject);
        assert_eq!(a.files[0].flag, Flag::Reject, "edit applied in memory");
        assert_eq!(a.undo_stack.len(), 1, "edit recorded an undo step automatically");
        assert!(a.redo_stack.is_empty(), "a fresh edit clears the redo branch");

        let _ = a.apply_undo_op(true);
        assert_eq!(a.files[0].flag, Flag::Unflagged, "undo reverts to the captured `before`");
    }

    #[test]
    fn fresh_edit_clears_redo_branch() {
        let mut a = app_with_catalog();
        a.file_ratings.insert("f1".into(), 3);
        let _ = a.edit_ratings(vec!["f1".into()], Some(5));
        let _ = a.apply_undo_op(true); // undo → redo now has one op
        assert_eq!(a.redo_stack.len(), 1);
        let _ = a.edit_ratings(vec!["f1".into()], Some(2)); // a new edit must drop the redo branch
        assert!(a.redo_stack.is_empty());
    }

    #[test]
    fn empty_stacks_are_no_ops() {
        let mut a = app_with_catalog();
        let _ = a.apply_undo_op(true);
        let _ = a.apply_undo_op(false);
        assert_eq!(a.undo_stack.len(), 0);
        assert_eq!(a.redo_stack.len(), 0);
    }

    #[test]
    fn delete_op_round_trips_and_arms_focus() {
        let mut a = app_with_catalog();
        a.undo_stack.push(UndoOp::SetDeleted { ids: vec!["f1".into()], deleted: true });

        let _ = a.apply_undo_op(true);
        assert_eq!(
            a.pending_focus_files.as_deref(),
            Some(&["f1".to_string()][..]),
            "undoing a delete arms a return-to-the-restored-photo focus",
        );
        assert!(matches!(a.redo_stack.last(), Some(UndoOp::SetDeleted { deleted: true, .. })));

        let _ = a.apply_undo_op(false);
        assert!(matches!(a.undo_stack.last(), Some(UndoOp::SetDeleted { deleted: true, .. })));
    }

    #[test]
    fn album_op_round_trips_and_arms_focus() {
        let mut a = app_with_catalog();
        a.undo_stack.push(UndoOp::Album {
            add: true,
            album_id: "al".into(),
            file_ids: vec!["f1".into()],
        });

        let _ = a.apply_undo_op(true);
        assert_eq!(a.pending_focus_files.as_deref(), Some(&["f1".to_string()][..]));
        assert!(matches!(a.redo_stack.last(), Some(UndoOp::Album { add: true, .. })));
    }

    // The headline behaviour: an edit in loupe auto-advances, so undo must return
    // the loupe to the photo it touched — not leave you parked on the next one.
    #[test]
    fn undo_returns_loupe_to_the_edited_photo() {
        let mut a = app_with_catalog();
        a.view_mode = crate::app::ViewMode::Loupe;
        a.loupe.idx = 2; // auto-advanced two past the edit
        a.pending_focus_files = Some(vec!["f2".into()]);
        let files =
            vec![file("f1", Flag::Unflagged), file("f2", Flag::Unflagged), file("f3", Flag::Unflagged)];

        let _ = a.update(Msg::FilesLoaded(files));
        assert_eq!(a.loupe.idx, 1, "loupe jumps back to f2 (the edited photo)");
        assert!(a.pending_focus_files.is_none(), "focus consumed");
    }

    #[test]
    fn loupe_nav_does_not_loop_at_either_end() {
        let mut a = app_with_catalog();
        a.view_mode = crate::app::ViewMode::Loupe;
        let files =
            vec![file("f1", Flag::Unflagged), file("f2", Flag::Unflagged), file("f3", Flag::Unflagged)];
        let _ = a.update(Msg::FilesLoaded(files));

        a.loupe.idx = 0;
        let _ = a.update(Msg::Navigate { dx: -1, dy: 0 });
        assert_eq!(a.loupe.idx, 0, "left at the first photo stays put (no wrap to last)");

        a.loupe.idx = 2;
        let _ = a.update(Msg::Navigate { dx: 1, dy: 0 });
        assert_eq!(a.loupe.idx, 2, "right at the last photo stays put (no wrap to first)");
    }

    #[test]
    fn scoped_loupe_steps_between_selected_only() {
        let mut a = app_with_catalog();
        a.view_mode = crate::app::ViewMode::Loupe;
        let files = vec![
            file("f1", Flag::Unflagged),
            file("f2", Flag::Unflagged),
            file("f3", Flag::Unflagged),
        ];
        let _ = a.update(Msg::FilesLoaded(files));
        // Scope to f1 and f3 (skip f2); start on f1.
        a.loupe.scope = vec![0, 2];
        a.loupe.idx = 0;

        let _ = a.update(Msg::Navigate { dx: 1, dy: 0 });
        assert_eq!(a.loupe.idx, 2, "next jumps over the unselected f2 to f3");
        let _ = a.update(Msg::Navigate { dx: 1, dy: 0 });
        assert_eq!(a.loupe.idx, 2, "at the end of the scope it stays put (no loop)");
        let _ = a.update(Msg::Navigate { dx: -1, dy: 0 });
        assert_eq!(a.loupe.idx, 0, "back to f1, still within the scope");
    }

    #[test]
    fn undo_reselects_the_edited_photo_in_grid() {
        let mut a = app_with_catalog();
        a.view_mode = crate::app::ViewMode::Browse;
        a.pending_focus_files = Some(vec!["f2".into()]);
        let files =
            vec![file("f1", Flag::Unflagged), file("f2", Flag::Unflagged), file("f3", Flag::Unflagged)];

        let _ = a.update(Msg::FilesLoaded(files));
        assert_eq!(a.anchor_idx, Some(1));
        assert!(a.grid_selected.contains("f2"), "the edited photo is re-selected");
    }

    #[test]
    fn focus_skips_a_photo_no_longer_present() {
        // Re-applying a delete (redo) targets a photo that's gone from the view; the
        // focus must simply fall through rather than mis-select.
        let mut a = app_with_catalog();
        a.view_mode = crate::app::ViewMode::Browse;
        a.anchor_idx = Some(0);
        a.pending_focus_files = Some(vec!["gone".into()]);
        let files = vec![file("f1", Flag::Unflagged), file("f2", Flag::Unflagged)];

        let _ = a.update(Msg::FilesLoaded(files));
        assert!(a.grid_selected.is_empty(), "absent focus target selects nothing");
        assert!(a.pending_focus_files.is_none(), "still consumed");
    }
}
