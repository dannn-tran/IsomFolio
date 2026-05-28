use iced::Task;
use isomfolio_core::models::SortField;

use super::super::{App, Msg};

impl App {
    pub(super) fn handle_criteria(&mut self, msg: Msg) -> Task<Msg> {
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

            Msg::ToggleCriteria => {
                self.criteria.show = !self.criteria.show;
                Task::none()
            }

            Msg::CriteriaTagInputChanged(s) => {
                self.criteria.tag_input = s;
                Task::none()
            }

            Msg::AddCriteriaTag => {
                let tag = self.criteria.tag_input.trim().to_string();
                self.criteria.tag_input.clear();
                if !tag.is_empty() && !self.criteria.tags.contains(&tag) {
                    self.criteria.tags.push(tag);
                }
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::RemoveCriteriaTag(tag) => {
                self.criteria.tags.retain(|t| t != &tag);
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::CriteriaDateFromChanged(s) => {
                self.criteria.date_from = s;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::CriteriaDateToChanged(s) => {
                self.criteria.date_to = s;
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::ToggleCriteriaExt(ext) => {
                if self.criteria.exts.contains(&ext) {
                    self.criteria.exts.remove(&ext);
                } else {
                    self.criteria.exts.insert(ext);
                }
                self.mark_smart_dirty();
                self.load_files_task()
            }

            Msg::ClearCriteria => {
                self.criteria.tags.clear();
                self.criteria.date_from.clear();
                self.criteria.date_to.clear();
                self.criteria.exts.clear();
                self.criteria.flag_filter = isomfolio_core::models::FlagFilter::All;
                self.criteria.rating_min = None;
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
