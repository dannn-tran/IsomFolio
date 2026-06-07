use iced::Task;

use super::LockUnwrap;
use super::super::{App, Msg, TagBrowserState};

impl App {
    pub(super) fn handle_tag_browser(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::OpenTagBrowser => {
                self.tag_browser = Some(TagBrowserState::default());
                self.load_tag_browser_task()
            }

            Msg::CloseTagBrowser => {
                self.tag_browser = None;
                Task::none()
            }

            Msg::TagBrowserLoaded(tags) => {
                if let Some(ref mut tb) = self.tag_browser {
                    tb.tags = tags;
                }
                Task::none()
            }

            Msg::TagBrowserFilterChanged(s) => {
                if let Some(ref mut tb) = self.tag_browser {
                    tb.filter = s;
                }
                Task::none()
            }

            Msg::TagBrowserRenameStart(tag) => {
                if let Some(ref mut tb) = self.tag_browser {
                    tb.rename = Some((tag.clone(), tag));
                    tb.delete_armed = None;
                }
                Task::none()
            }

            Msg::TagBrowserRenameChanged(s) => {
                if let Some(ref mut tb) = self.tag_browser {
                    if let Some((_, ref mut input)) = tb.rename {
                        *input = s;
                    }
                }
                Task::none()
            }

            Msg::TagBrowserRenameConfirm => {
                let Some(ref tb) = self.tag_browser else {
                    return Task::none();
                };
                let Some((ref old, ref new_name)) = tb.rename else {
                    return Task::none();
                };
                let old = old.clone();
                let new_name = new_name.trim().to_string();
                if new_name.is_empty() || new_name == old {
                    if let Some(ref mut tb) = self.tag_browser {
                        tb.rename = None;
                    }
                    return Task::none();
                }
                if let Some(ref mut tb) = self.tag_browser {
                    tb.rename = None;
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.rename_prefixed_tags(&old, &new_name).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::TagBrowserTagRenamed, Msg::DbError),
                )
            }

            Msg::TagBrowserRenameCancel => {
                if let Some(ref mut tb) = self.tag_browser {
                    tb.rename = None;
                }
                Task::none()
            }

            Msg::TagBrowserDeleteArm(tag) => {
                if let Some(ref mut tb) = self.tag_browser {
                    tb.delete_armed = Some(tag);
                    tb.rename = None;
                }
                Task::none()
            }

            Msg::TagBrowserDeleteConfirm => {
                let Some(ref tb) = self.tag_browser else {
                    return Task::none();
                };
                let Some(ref tag) = tb.delete_armed else {
                    return Task::none();
                };
                let tag = tag.clone();
                if let Some(ref mut tb) = self.tag_browser {
                    tb.delete_armed = None;
                }
                let Some(conn) = self.catalog.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let g = conn.lock_unwrap();
                        g.delete_tag_with_descendants(&tag).err().map(|e| e.to_string())
                    },
                    |e| e.map_or(Msg::TagBrowserTagDeleted, Msg::DbError),
                )
            }

            Msg::TagBrowserDeleteCancel => {
                if let Some(ref mut tb) = self.tag_browser {
                    tb.delete_armed = None;
                }
                Task::none()
            }

            Msg::TagBrowserTagRenamed | Msg::TagBrowserTagDeleted => {
                self.detail.file_id = None;
                let t1 = self.load_tag_browser_task();
                let t2 = self.load_all_tags_task();
                let t3 = self.maybe_load_detail();
                Task::batch([t1, t2, t3])
            }

            other => {
                debug_assert!(false, "handle_tag_browser received misrouted message: {other:?}");
                Task::none()
            }
        }
    }
}
