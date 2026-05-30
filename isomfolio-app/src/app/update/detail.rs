use iced::Task;

use super::LockUnwrap;
use super::super::{App, Msg, UndoOp};

impl App {
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

            Msg::DetailLoaded {
                file_id,
                tags,
                tag_confidence,
                pending_tags,
                rating,
                label,
                title,
                exif_tech,
            } => {
                self.detail.file_id = Some(file_id);
                self.detail.batch_file_ids.clear();
                self.detail.tags = tags;
                self.detail.tag_confidence = tag_confidence;
                self.detail.pending_tags = pending_tags;
                self.detail.rating = rating;
                self.detail.label = label;
                self.detail.title = title;
                self.detail.exif_tech = exif_tech;
                self.load_all_tags_task()
            }

            Msg::BatchDetailLoaded { file_ids, tags } => {
                self.detail.file_id = None;
                self.detail.batch_file_ids = file_ids;
                self.detail.tags = tags;
                self.detail.rating = None;
                self.detail.label = None;
                self.detail.title = None;
                self.detail.exif_tech = None;
                self.load_all_tags_task()
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
                self.undo_stack.push(UndoOp::AddedTag { file_ids, tag: tag.clone() });
                self.redo_stack.clear();
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
                self.undo_stack.push(UndoOp::AddedTag { file_ids, tag: tag.clone() });
                self.redo_stack.clear();
                if !self.detail.batch_file_ids.is_empty() {
                    self.batch_add_tag_task(tag)
                } else {
                    self.save_detail_tags_task()
                }
            }

            Msg::RemoveDetailTag(tag) => {
                self.detail.tags.retain(|t| t != &tag);
                let file_ids = self.current_detail_file_ids();
                self.undo_stack.push(UndoOp::RemovedTag { file_ids, tag: tag.clone() });
                self.redo_stack.clear();
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

            Msg::AcceptPendingTag(tag) => {
                let Some(ref fid) = self.detail.file_id else { return Task::none() };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.detail.pending_tags.retain(|(t, _)| t != &tag);
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.accept_pending_tag(&fid, &tag).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::PendingTagsUpdated, Msg::DbError),
                )
            }

            Msg::RejectPendingTag(tag) => {
                let Some(ref fid) = self.detail.file_id else { return Task::none() };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.detail.pending_tags.retain(|(t, _)| t != &tag);
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.reject_pending_tag(&fid, &tag).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::PendingTagsUpdated, Msg::DbError),
                )
            }

            Msg::AcceptAllPending => {
                let Some(ref fid) = self.detail.file_id else { return Task::none() };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.detail.pending_tags.clear();
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.accept_all_pending(&fid).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::PendingTagsUpdated, Msg::DbError),
                )
            }

            Msg::RejectAllPending => {
                let Some(ref fid) = self.detail.file_id else { return Task::none() };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.detail.pending_tags.clear();
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.reject_all_pending(&fid).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::PendingTagsUpdated, Msg::DbError),
                )
            }

            Msg::PendingTagsUpdated => {
                self.detail.file_id = None;
                let mut tasks = vec![self.maybe_load_detail(), self.refresh_pending_total_task()];
                if matches!(
                    self.selected_item,
                    super::super::SidebarItem::Suggestions,
                ) && matches!(self.suggestion_view, super::super::SuggestionView::Tag)
                {
                    tasks.push(self.load_pending_tag_groups_task());
                }
                Task::batch(tasks)
            }

            Msg::AcceptAllInView => {
                let ids: Vec<String> = self.files.iter().map(|f| f.id.clone()).collect();
                if ids.is_empty() {
                    return Task::none();
                }
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.pending_counts_by_id.clear();
                self.detail.pending_tags.clear();
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.accept_all_pending_batch(&ids).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::Reload, Msg::DbError),
                )
            }

            Msg::RejectAllInView => {
                let ids: Vec<String> = self.files.iter().map(|f| f.id.clone()).collect();
                if ids.is_empty() {
                    return Task::none();
                }
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.pending_counts_by_id.clear();
                self.detail.pending_tags.clear();
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.reject_all_pending_batch(&ids).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::Reload, Msg::DbError),
                )
            }

            Msg::PendingCountsLoaded { counts, total } => {
                self.pending_counts_by_id = counts;
                self.pending_tag_file_count = total;
                Task::none()
            }

            Msg::PendingTotalLoaded(total) => {
                self.pending_tag_file_count = total;
                Task::none()
            }

            Msg::SetSuggestionView(view) => {
                self.suggestion_view = view;
                // Loading the right data is handled by SidebarItemClicked when entering
                // Suggestions. Switching mid-view loads here.
                if matches!(self.selected_item, super::super::SidebarItem::Suggestions) {
                    match view {
                        super::super::SuggestionView::Tag => self.load_pending_tag_groups_task(),
                        super::super::SuggestionView::Photo => self.load_files_task(),
                    }
                } else {
                    Task::none()
                }
            }

            Msg::PendingTagGroupsLoaded(groups) => {
                self.pending_tag_groups = groups;
                self.status = format!("{} pending tag(s)", self.pending_tag_groups.len());
                let samples: Vec<(String, String)> = self
                    .pending_tag_groups
                    .iter()
                    .flat_map(|g| g.sample_files.iter().take(4).cloned())
                    .collect();
                self.enqueue_thumbnails_for_ids(&samples);
                Task::none()
            }

            Msg::AcceptPendingTagGlobally(tag) => {
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                let tag_clone = tag.clone();
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.accept_pending_tag_globally(&tag_clone)
                            .err()
                            .map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::PendingTagsUpdated, Msg::DbError),
                )
            }

            Msg::RejectPendingTagGlobally(tag) => {
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                let tag_clone = tag.clone();
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.reject_pending_tag_globally(&tag_clone)
                            .err()
                            .map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::PendingTagsUpdated, Msg::DbError),
                )
            }

            Msg::SetDetailRating(n) => {
                let new_rating = if self.detail.rating == Some(n) { None } else { Some(n) };
                self.detail.rating = new_rating;
                let Some(ref fid) = self.detail.file_id else { return Task::none() };
                let fid = fid.clone();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.set_file_rating(&fid, new_rating).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::NoOp, Msg::DbError),
                )
            }

            Msg::SetFlag(flag) => {
                let ids: Vec<String> =
                    if matches!(self.view_mode, super::super::ViewMode::Loupe) {
                        self.files
                            .get(self.loupe.idx)
                            .map(|f| vec![f.id.clone()])
                            .unwrap_or_default()
                    } else {
                        self.grid_selected.iter().cloned().collect()
                    };
                if ids.is_empty() {
                    return Task::none();
                }
                let before: Vec<(String, isomfolio_core::models::Flag)> = ids
                    .iter()
                    .filter_map(|id| {
                        self.files.iter().find(|f| &f.id == id).map(|f| (id.clone(), f.flag))
                    })
                    .collect();
                for id in &ids {
                    if let Some(f) = self.files.iter_mut().find(|f| &f.id == id) {
                        f.flag = flag;
                    }
                }
                self.undo_stack.push(UndoOp::SetFlags { before });
                self.redo_stack.clear();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                let db_task = Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.set_files_flag(&ids, flag).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::FlagsApplied, Msg::DbError),
                );
                if matches!(self.view_mode, super::super::ViewMode::Loupe)
                    && self.app_settings.auto_advance_on_flag
                {
                    Task::batch([db_task, Task::done(Msg::Navigate { dx: 1, dy: 0 })])
                } else {
                    db_task
                }
            }

            Msg::FlagsApplied => {
                if self.filters.hide_rejects
                    || self.filters.flag_filter != isomfolio_core::models::FlagFilter::All
                {
                    self.load_files_task()
                } else {
                    Task::none()
                }
            }

            Msg::SetRating(rating) => {
                let ids: Vec<String> =
                    if matches!(self.view_mode, super::super::ViewMode::Loupe) {
                        self.files
                            .get(self.loupe.idx)
                            .map(|f| vec![f.id.clone()])
                            .unwrap_or_default()
                    } else {
                        self.grid_selected.iter().cloned().collect()
                    };
                if ids.is_empty() {
                    return Task::none();
                }
                let before: Vec<(String, Option<i32>)> = ids
                    .iter()
                    .map(|id| (id.clone(), self.file_ratings.get(id).copied()))
                    .collect();
                for id in &ids {
                    match rating {
                        Some(r) if r > 0 => {
                            self.file_ratings.insert(id.clone(), r);
                        }
                        _ => {
                            self.file_ratings.remove(id);
                        }
                    }
                }
                if ids.len() == 1 && self.detail.file_id.as_deref() == Some(ids[0].as_str()) {
                    self.detail.rating = rating;
                }
                self.undo_stack.push(UndoOp::SetRatings { before });
                self.redo_stack.clear();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.set_files_rating(&ids, rating).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::RatingsApplied, Msg::DbError),
                )
            }

            Msg::RatingsApplied => {
                if self.filters.rating_min.is_some() { self.load_files_task() } else { Task::none() }
            }

            Msg::RatingsLoaded(map) => {
                self.file_ratings = map;
                Task::none()
            }

            Msg::ToggleHideRejects => {
                self.filters.hide_rejects = !self.filters.hide_rejects;
                self.load_files_task()
            }

            Msg::SetFlagFilter(filter) => {
                self.filters.flag_filter = filter;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::SetRatingFilter(min) => {
                self.filters.rating_min = min;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::SetLocationFilter(val) => {
                self.filters.has_location = val;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::Undo => self.apply_undo_op(true),
            Msg::Redo => self.apply_undo_op(false),

            Msg::UndoApplied => {
                let t1 = self.load_files_task();
                let t2 = self.maybe_load_detail();
                let t3 = self.load_ratings_task();
                Task::batch([t1, t2, t3])
            }

            _ => Task::none(),
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
        let Some(conn) = self.catalog.clone() else { return Task::none() };

        match op {
            UndoOp::AddedTag { file_ids, tag } => {
                let inverse =
                    UndoOp::RemovedTag { file_ids: file_ids.clone(), tag: tag.clone() };
                if is_undo {
                    self.redo_stack.push(inverse);
                } else {
                    self.undo_stack.push(inverse);
                }
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.remove_tag_from_files(&file_ids, &tag).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::UndoApplied, Msg::DbError),
                )
            }
            UndoOp::RemovedTag { file_ids, tag } => {
                let inverse =
                    UndoOp::AddedTag { file_ids: file_ids.clone(), tag: tag.clone() };
                if is_undo {
                    self.redo_stack.push(inverse);
                } else {
                    self.undo_stack.push(inverse);
                }
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.add_tag_to_files(&file_ids, &tag).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::UndoApplied, Msg::DbError),
                )
            }
            UndoOp::SetRatings { before } => {
                let after: Vec<(String, Option<i32>)> = before
                    .iter()
                    .map(|(id, _)| (id.clone(), self.file_ratings.get(id).copied()))
                    .collect();
                let inverse = UndoOp::SetRatings { before: after };
                if is_undo {
                    self.redo_stack.push(inverse);
                } else {
                    self.undo_stack.push(inverse);
                }
                for (id, rating) in &before {
                    match rating {
                        Some(r) if *r > 0 => {
                            self.file_ratings.insert(id.clone(), *r);
                        }
                        _ => {
                            self.file_ratings.remove(id);
                        }
                    }
                }
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        for (id, rating) in &before {
                            if let Err(e) = g.set_file_rating(id, *rating) {
                                return Some(e.to_string());
                            }
                        }
                        None
                    },
                    |e| e.map_or(Msg::UndoApplied, Msg::DbError),
                )
            }
            UndoOp::SetFlags { before } => {
                let after: Vec<(String, isomfolio_core::models::Flag)> = before
                    .iter()
                    .filter_map(|(id, _)| {
                        self.files.iter().find(|f| &f.id == id).map(|f| (id.clone(), f.flag))
                    })
                    .collect();
                let inverse = UndoOp::SetFlags { before: after };
                if is_undo {
                    self.redo_stack.push(inverse);
                } else {
                    self.undo_stack.push(inverse);
                }
                for (id, flag) in &before {
                    if let Some(f) = self.files.iter_mut().find(|f| &f.id == id) {
                        f.flag = *flag;
                    }
                }
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        for (id, flag) in &before {
                            if let Err(e) = g.set_file_flag(id, *flag) {
                                return Some(e.to_string());
                            }
                        }
                        None
                    },
                    |e| e.map_or(Msg::UndoApplied, Msg::DbError),
                )
            }
        }
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
