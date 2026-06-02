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
                if self.filters.flags.is_active() {
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
                if self.filters.rating.is_active() { self.load_files_task() } else { Task::none() }
            }

            Msg::RatingsLoaded(map) => {
                self.file_ratings = map;
                Task::none()
            }

            Msg::LabelsLoaded(map) => {
                self.file_labels = map;
                Task::none()
            }

            Msg::BurstSizesLoaded(map) => {
                self.file_burst_sizes = map;
                Task::none()
            }

            Msg::SetColorLabel(color) => {
                let ids: Vec<String> =
                    if matches!(self.view_mode, super::super::ViewMode::Loupe) {
                        self.files.get(self.loupe.idx).map(|f| vec![f.id.clone()]).unwrap_or_default()
                    } else {
                        self.grid_selected.iter().cloned().collect()
                    };
                if ids.is_empty() {
                    return Task::none();
                }
                // Pressing the same colour again clears it (toggle off).
                let effective = match &color {
                    Some(c) if ids.iter().all(|id| self.file_labels.get(id) == Some(c)) => None,
                    other => other.clone(),
                };
                let before: Vec<(String, Option<String>)> = ids
                    .iter()
                    .map(|id| (id.clone(), self.file_labels.get(id).cloned()))
                    .collect();
                for id in &ids {
                    match &effective {
                        Some(c) => { self.file_labels.insert(id.clone(), c.clone()); }
                        None => { self.file_labels.remove(id); }
                    }
                }
                self.undo_stack.push(UndoOp::SetLabels { before });
                self.redo_stack.clear();
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                let label_owned = effective.clone();
                let reload = self.filters.color.is_some();
                let save = Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.set_files_label(&ids, label_owned.as_deref()).err().map(|e| e.to_string())
                    },
                    |e| match e { Some(err) => Msg::DbError(err), None => Msg::NoOp },
                );
                if reload {
                    Task::batch([save, self.load_files_task()])
                } else {
                    save
                }
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
                let t1 = self.load_files_task();
                let t2 = self.maybe_load_detail();
                let t3 = self.load_ratings_task();
                let t4 = self.load_labels_task();
                Task::batch([t1, t2, t3, t4])
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
            UndoOp::SetLabels { before } => {
                let after: Vec<(String, Option<String>)> = before
                    .iter()
                    .map(|(id, _)| (id.clone(), self.file_labels.get(id).cloned()))
                    .collect();
                let inverse = UndoOp::SetLabels { before: after };
                if is_undo {
                    self.redo_stack.push(inverse);
                } else {
                    self.undo_stack.push(inverse);
                }
                for (id, label) in &before {
                    match label {
                        Some(l) => { self.file_labels.insert(id.clone(), l.clone()); }
                        None => { self.file_labels.remove(id); }
                    }
                }
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        for (id, label) in &before {
                            if let Err(e) = g.set_files_label(&[id.clone()], label.as_deref()) {
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
