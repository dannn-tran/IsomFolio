mod albums;
mod catalog;
mod criteria;
mod detail;
mod extensions;
mod navigation;
mod scan;
mod settings;
mod tag_browser;

use iced::Task;
use isomfolio_core::models::ThumbnailState;

use super::{
    unix_to_date_str, AlbumKind, App, Msg, SidebarItem,
};

pub(super) use super::LockUnwrap;

impl App {
    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            // — catalog & welcome —
            Msg::CatalogReady
            | Msg::OpenCatalog(_)
            | Msg::PickOpenCatalog
            | Msg::OpenCatalogPicked(_)
            | Msg::SelectRecentCatalog(_)
            | Msg::OpenSelectedRecentCatalog
            | Msg::ReturnToWelcome
            | Msg::ShowNewCatalogModal
            | Msg::HideNewCatalogModal
            | Msg::PickNewCatalogDir
            | Msg::NewCatalogDirPicked(_)
            | Msg::NewCatalogNameChanged(_)
            | Msg::ConfirmNewCatalog => self.handle_catalog_msg(msg),

            // — extensions & face clustering —
            Msg::ExtensionsDiscovered(_)
            | Msg::RunExtension { .. }
            | Msg::ExtensionProgress { .. }
            | Msg::ExtensionBatchProgress { .. }
            | Msg::ExtensionBatchDone { .. }
            | Msg::ExtensionRestarted { .. }
            | Msg::RunFaceClustering { .. }
            | Msg::FaceClusteringDone(_)
            | Msg::FaceClustersLoaded(_)
            | Msg::FaceCropsReady(_)
            | Msg::OpenPeopleView
            | Msg::RenameFaceCluster(_)
            | Msg::RenameFaceClusterInputChanged(_)
            | Msg::ConfirmRenameFaceCluster
            | Msg::MergeFaceClusters(_, _)
            | Msg::RemoveFileFromFaceCluster(_, _) => self.handle_extension_msg(msg),

            // — scanning & file watching —
            Msg::ScanPickFolder
            | Msg::ScanDialogDone(_)
            | Msg::ScanStart(_)
            | Msg::ScanComplete { .. }
            | Msg::RequestRemoveFolder(_)
            | Msg::CancelRemoveFolder
            | Msg::RemoveFolder(_)
            | Msg::FolderRemoved
            | Msg::RescanFolder(_)
            | Msg::FileWatcherEvent(_)
            | Msg::FlushFileEvents(_) => self.handle_scan_msg(msg),

            // — detail panel, tags, ratings, flags, undo —
            Msg::ToggleDetail
            | Msg::DetailLoaded { .. }
            | Msg::BatchDetailLoaded { .. }
            | Msg::BatchTagsChanged
            | Msg::DetailTagInputChanged(_)
            | Msg::AddDetailTag
            | Msg::AddDetailTagDirect(_)
            | Msg::RemoveDetailTag(_)
            | Msg::AllTagsLoaded(_)
            | Msg::TagsSavedResult(_, _)
            | Msg::RepeatLastTag
            | Msg::AcceptPendingTag(_)
            | Msg::RejectPendingTag(_)
            | Msg::AcceptAllPending
            | Msg::RejectAllPending
            | Msg::PendingTagsUpdated
            | Msg::SetDetailRating(_)
            | Msg::SetFlag(_)
            | Msg::FlagsApplied
            | Msg::SetRating(_)
            | Msg::RatingsApplied
            | Msg::RatingsLoaded(_)
            | Msg::ToggleHideRejects
            | Msg::SetFlagFilter(_)
            | Msg::SetRatingFilter(_)
            | Msg::SetLocationFilter(_)
            | Msg::Undo
            | Msg::Redo
            | Msg::UndoApplied => self.handle_detail_msg(msg),

            // — navigation, mouse, loupe, compare, context menu —
            Msg::TileSizeUp
            | Msg::TileSizeDown
            | Msg::Navigate { .. }
            | Msg::OpenLoupe
            | Msg::TogglePreview
            | Msg::SidebarResizeStart
            | Msg::MouseMoved(_)
            | Msg::MouseRightClicked
            | Msg::MousePressed
            | Msg::MouseReleased
            | Msg::ModifiersChanged(_)
            | Msg::EscapePressed
            | Msg::Scrolled { .. }
            | Msg::DragHoverAlbum(_)
            | Msg::OpenContextMenu(_, _)
            | Msg::OpenFaceClusterMenu(_)
            | Msg::ToggleAddToAlbumSubmenu
            | Msg::CloseContextMenu
            | Msg::HoverSidebarEntityStart(_)
            | Msg::HoverSidebarEntityEnd(_)
            | Msg::ToggleShortcutHelp
            | Msg::OpenMenuDropdown(_)
            | Msg::CloseMenuDropdown
            | Msg::LoupeFullResLoaded { .. }
            | Msg::LoupePrefetchLoaded { .. }
            | Msg::SelectAll
            | Msg::DeselectAll
            | Msg::OpenCompare
            | Msg::CompareFullResLoaded { .. }
            | Msg::ShowInFinder(_)
            | Msg::SidebarScrolled(_) => self.handle_navigation_msg(msg),

            // — albums —
            Msg::DroppedToAlbum(_, _)
            | Msg::DropCompleted
            | Msg::AddSelectionToAlbum(_)
            | Msg::DuplicateAlbum(_)
            | Msg::StartCreateAlbum
            | Msg::CreateAlbumInputChanged(_)
            | Msg::ConfirmCreateAlbum
            | Msg::CancelCreateAlbum
            | Msg::AlbumCreated
            | Msg::AlbumRenamed
            | Msg::FilesRemovedFromAlbum
            | Msg::StartRenameAlbum(_)
            | Msg::RenameAlbumInputChanged(_)
            | Msg::ConfirmRenameAlbum
            | Msg::RequestDeleteAlbum(_)
            | Msg::CancelDeleteAlbum
            | Msg::DeleteAlbum(_)
            | Msg::AlbumDeleted
            | Msg::RemoveFromAlbum
            | Msg::CancelRemoveFromAlbum
            | Msg::ConfirmRemoveFromAlbum
            | Msg::SaveAsSmartAlbum
            | Msg::SmartAlbumNameChanged(_)
            | Msg::ConfirmSmartAlbum
            | Msg::SmartAlbumUpdated
            | Msg::UpdateSmartAlbum => self.handle_album_msg(msg),

            // — search & filter criteria —
            Msg::SortFieldCycle
            | Msg::SortDirToggle
            | Msg::SortCycleAll
            | Msg::SearchChanged(_)
            | Msg::ToggleCriteria
            | Msg::CriteriaTagInputChanged(_)
            | Msg::AddCriteriaTag
            | Msg::RemoveCriteriaTag(_)
            | Msg::CriteriaDateFromChanged(_)
            | Msg::CriteriaDateToChanged(_)
            | Msg::ToggleCriteriaExt(_)
            | Msg::ClearCriteria => self.handle_criteria(msg),

            // — settings panel —
            Msg::OpenSettings
            | Msg::CloseSettings
            | Msg::SettingsConfigChanged { .. }
            | Msg::SaveSettings
            | Msg::InstallExtensionPickFile
            | Msg::ExtensionPackagePicked(_)
            | Msg::ExtensionInstalled(_)
            | Msg::ExtensionInstallFailed(_)
            | Msg::UninstallExtension(_)
            | Msg::SetPreferredExtension { .. } => self.handle_settings(msg),

            // — tag browser —
            Msg::OpenTagBrowser
            | Msg::CloseTagBrowser
            | Msg::TagBrowserLoaded(_)
            | Msg::TagBrowserFilterChanged(_)
            | Msg::TagBrowserRenameStart(_)
            | Msg::TagBrowserRenameChanged(_)
            | Msg::TagBrowserRenameConfirm
            | Msg::TagBrowserRenameCancel
            | Msg::TagBrowserDeleteArm(_)
            | Msg::TagBrowserDeleteConfirm
            | Msg::TagBrowserDeleteCancel
            | Msg::TagBrowserTagRenamed
            | Msg::TagBrowserTagDeleted => self.handle_tag_browser(msg),

            // — inline: sidebar & file loading —
            Msg::SidebarItemClicked(item) => {
                if let SidebarItem::Album(ref id) = item {
                    if let Some(album) = self.albums.iter().find(|a| &a.id == id) {
                        if let AlbumKind::Smart(ref q) = album.kind {
                            self.criteria.tags = q.tags.clone();
                            self.criteria.date_from =
                                q.date_from.map(unix_to_date_str).unwrap_or_default();
                            self.criteria.date_to =
                                q.date_to.map(unix_to_date_str).unwrap_or_default();
                            self.criteria.exts = q.extensions.iter().cloned().collect();
                            self.search_text = q.text.clone().unwrap_or_default();
                            self.criteria.has_location = q.has_location;
                            self.criteria.show = true;
                        }
                    }
                }
                self.selected_item = item;
                if matches!(self.view_mode, super::ViewMode::People) {
                    self.view_mode = super::ViewMode::Browse;
                }
                self.files.clear();
                self.file_ratings.clear();
                self.scroll_y = 0.0;
                self.loupe.idx = 0;
                self.grid_selected.clear();
                self.drag.state = None;
                self.drag.ids.clear();
                self.criteria.save_smart_input = None;
                self.detail.file_id = None;
                self.remove_from_album_pending = false;
                self.smart_album_dirty = false;
                self.load_files_task()
            }

            Msg::FilesLoaded(files) => {
                self.files = files;
                self.enqueue_thumbnails();
                self.status = format!("{} photo(s)", self.files.len());
                let t1 = self.maybe_load_detail();
                let t2 = self.load_ratings_task();
                Task::batch([t1, t2])
            }

            Msg::SidebarLoaded { folders, albums, album_counts } => {
                self.folders = folders;
                self.albums = albums;
                self.album_counts = album_counts;
                self.start_watchers_for_folders();
                if let Some(id) = self.pending_album_select.take() {
                    Task::done(Msg::SidebarItemClicked(SidebarItem::Album(id)))
                } else if self.selected_item == SidebarItem::AllFiles {
                    if let Some((path, _, _)) = self.folders.first() {
                        Task::done(Msg::SidebarItemClicked(SidebarItem::Folder(path.clone())))
                    } else if let Some(album) = self.albums.first() {
                        Task::done(Msg::SidebarItemClicked(SidebarItem::Album(album.id.clone())))
                    } else {
                        self.load_files_task()
                    }
                } else {
                    Task::none()
                }
            }

            Msg::Reload => {
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            // — inline: thumbnails —
            Msg::ThumbnailCompleted { file_id, path } => {
                self.thumbnails.insert(file_id.clone(), ThumbnailState::Ready(path.clone()));
                self.thumb_ctx.pending = self.thumb_ctx.pending.saturating_sub(1);
                let clear_task = if self.thumb_ctx.pending == 0 && self.thumb_ctx.total > 0 {
                    let gen = self.thumb_ctx.done_gen;
                    Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                std::thread::sleep(std::time::Duration::from_secs(2));
                                gen
                            })
                            .await
                            .unwrap_or(gen)
                        },
                        Msg::ClearThumbnailProgress,
                    )
                } else {
                    Task::none()
                };
                let fid2 = file_id.clone();
                let load_task = Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            image::open(&path).ok().map(|img| {
                                let rgba = img.into_rgba8();
                                let (w, h) = (rgba.width(), rgba.height());
                                (fid2, iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw()))
                            })
                        })
                        .await
                        .ok()
                        .flatten()
                    },
                    |res| match res {
                        Some((fid, handle)) => Msg::ThumbnailHandleReady { file_id: fid, handle },
                        None => Msg::NoOp,
                    },
                );
                Task::batch([load_task, clear_task])
            }

            Msg::ThumbnailFailed { file_id } => {
                self.thumbnails.insert(file_id, ThumbnailState::Failed(0));
                self.thumb_ctx.pending = self.thumb_ctx.pending.saturating_sub(1);
                if self.thumb_ctx.pending == 0 && self.thumb_ctx.total > 0 {
                    let gen = self.thumb_ctx.done_gen;
                    Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                std::thread::sleep(std::time::Duration::from_secs(2));
                                gen
                            })
                            .await
                            .unwrap_or(gen)
                        },
                        Msg::ClearThumbnailProgress,
                    )
                } else {
                    Task::none()
                }
            }

            Msg::ThumbnailHandleReady { file_id, handle } => {
                self.thumb_ctx.handles.insert(file_id, handle);
                Task::none()
            }

            Msg::ClearThumbnailProgress(gen) => {
                if gen == self.thumb_ctx.done_gen {
                    self.thumb_ctx.total = 0;
                }
                Task::none()
            }

            Msg::SearchDebounceTimer { id, text } => {
                if id != self.search_debounce_id {
                    return Task::none();
                }
                self.search_text = text;
                self.scroll_y = 0.0;
                self.files.clear();
                self.grid_selected.clear();
                self.load_files_task()
            }

            Msg::DbError(e) => {
                self.status = format!("Error: {e}");
                Task::none()
            }

            Msg::NoOp => Task::none(),
        }
    }

    pub(super) fn mark_smart_dirty(&mut self) {
        if self.current_album_is_smart() {
            self.smart_album_dirty = true;
        }
    }
}
