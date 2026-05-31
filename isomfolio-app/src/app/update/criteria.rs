use iced::Task;
use isomfolio_core::models::SortField;

use super::super::{App, Msg};

impl App {
    pub(super) fn handle_filters(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::SortFieldCycle => {
                self.sort_by = next_sort_field(self.sort_by);
                self.load_files_task()
            }

            Msg::SortDirToggle => {
                self.sort_asc = !self.sort_asc;
                self.load_files_task()
            }

            Msg::SortCycleAll => {
                if self.sort_asc {
                    self.sort_asc = false;
                } else {
                    self.sort_by = next_sort_field(self.sort_by);
                    self.sort_asc = true;
                }
                self.load_files_task()
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
                self.filters.show = !self.filters.show;
                Task::none()
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
                self.filters.date_from.clear();
                self.filters.date_to.clear();
                self.filters.exts.clear();
                self.filters.flag_filter = isomfolio_core::models::FlagFilter::All;
                self.filters.rating_min = None;
                self.filters.has_location = None;
                self.filters.person = None;
                self.filters.added_within_days = None;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            _ => Task::none(),
        }
    }
}

fn next_sort_field(f: SortField) -> SortField {
    match f {
        SortField::Name => SortField::Date,
        SortField::Date => SortField::Size,
        SortField::Size => SortField::Ext,
        SortField::Ext => SortField::Name,
    }
}
