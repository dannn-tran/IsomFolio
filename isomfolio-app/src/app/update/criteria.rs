use iced::Task;
use super::super::{App, Msg};

impl App {
    pub(super) fn handle_filters(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::SortDirToggle => {
                self.sort_asc = !self.sort_asc;
                self.load_files_task()
            }

            Msg::SetSortField(field) => {
                if self.sort_by == field {
                    return Task::none();
                }
                self.sort_by = field;
                self.load_files_task()
            }

            Msg::SetGridLayout(layout) => {
                // Pure presentation change — no reload. Keep the anchor visible
                // since row geometry changes underneath it.
                self.grid_layout = layout;
                if let Some(idx) = self.anchor_idx {
                    return self.scroll_to_index(idx);
                }
                Task::none()
            }

            Msg::SearchChanged(text) => {
                self.mark_smart_dirty();
                self.search_debounce_id += 1;
                let id = self.search_debounce_id;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            std::thread::sleep(std::time::Duration::from_millis(300));
                            (id, text)
                        })
                        .await
                        .unwrap_or((id, String::new()))
                    },
                    |(id, text)| Msg::SearchDebounceTimer { id, text },
                )
            }

            Msg::ToggleFilterPanel => {
                use crate::app::SidebarSection;
                if self.collapsed_sections.contains(&SidebarSection::Filters) {
                    self.collapsed_sections.remove(&SidebarSection::Filters);
                } else {
                    self.collapsed_sections.insert(SidebarSection::Filters);
                }
                Task::none()
            }

            Msg::ToggleCollapseBursts => {
                self.collapse_bursts = !self.collapse_bursts;
                // Per-stack expands only mean something while collapsed; reset
                // them whenever the global toggle flips so state can't go stale.
                self.expanded_bursts.clear();
                self.load_files_task()
            }

            Msg::FilterTagInputChanged(s) => {
                self.filters.tag_input = s;
                Task::none()
            }

            Msg::AddFilterTag => {
                let tag = self.filters.tag_input.trim().to_string();
                self.filters.tag_input.clear();
                if !tag.is_empty() && !self.filters.tags.contains(&tag) {
                    self.filters.tags.push(tag);
                }
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::RemoveFilterTag(tag) => {
                self.filters.tags.retain(|t| t != &tag);
                self.filters.exclude_tags.retain(|t| t != &tag);
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::ToggleFilterTagNegate(tag) => {
                // Cycle a chip between include (tags) and exclude (exclude_tags).
                if let Some(pos) = self.filters.tags.iter().position(|t| t == &tag) {
                    self.filters.tags.remove(pos);
                    if !self.filters.exclude_tags.contains(&tag) {
                        self.filters.exclude_tags.push(tag);
                    }
                } else if let Some(pos) = self.filters.exclude_tags.iter().position(|t| t == &tag) {
                    self.filters.exclude_tags.remove(pos);
                    if !self.filters.tags.contains(&tag) {
                        self.filters.tags.push(tag);
                    }
                }
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::SetTagMatch(mode) => {
                self.filters.tag_match = mode;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::FilterDateFromChanged(s) => {
                self.filters.date_from = s;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::FilterDateToChanged(s) => {
                self.filters.date_to = s;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::SetPersonFilter(cluster) => {
                self.filters.person = cluster;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::SetAddedWithinFilter(days) => {
                self.filters.added_within_days = days;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::SetCameraFilter(camera) => {
                self.filters.camera = camera;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::SetColorFilter(color) => {
                self.filters.color = color;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::SetDatePreset(preset) => {
                let (from, to) = super::super::date_preset_range(preset);
                self.filters.date_from = from;
                self.filters.date_to = to;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::ToggleFilterFileType(ext) => {
                if self.filters.exts.contains(&ext) {
                    self.filters.exts.remove(&ext);
                } else {
                    self.filters.exts.insert(ext);
                }
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::ClearFilters => {
                self.filters.tags.clear();
                self.filters.exclude_tags.clear();
                self.filters.tag_match = isomfolio_core::models::TagMatch::All;
                self.filters.date_from.clear();
                self.filters.date_to.clear();
                self.filters.exts.clear();
                self.filters.flags = isomfolio_core::models::FlagSelection::default();
                self.filters.rating = isomfolio_core::models::RatingFilter::Any;
                self.filters.has_location = None;
                self.filters.person = None;
                self.filters.added_within_days = None;
                self.filters.camera = None;
                self.filters.color = None;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            other => {
                debug_assert!(false, "handle_filters received misrouted message: {other:?}");
                Task::none()
            }
        }
    }
}
