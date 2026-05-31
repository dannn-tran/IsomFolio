mod albums;
mod catalog;
mod criteria;
mod detail;
mod extensions;
mod navigation;
mod sync;
mod settings;
mod tag_browser;

use iced::Task;
use isomfolio_core::models::ThumbnailState;

use super::{
    unix_to_date_str, AlbumKind, App, ExportMode, Msg, SidebarItem,
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
            | Msg::ShowNewCatalogModal
            | Msg::HideNewCatalogModal
            | Msg::PickNewCatalogDir
            | Msg::NewCatalogDirPicked(_)
            | Msg::NewCatalogNameChanged(_)
            | Msg::ConfirmNewCatalog => self.handle_catalog_msg(msg),

            // — extensions & face clustering —
            Msg::ExtensionsDiscovered(..)
            | Msg::RunExtension { .. }
            | Msg::ExtensionProgress { .. }
            | Msg::ExtensionBatchProgress { .. }
            | Msg::ExtensionBatchDone { .. }
            | Msg::ExtensionRestarted { .. }
            | Msg::RunFaceClustering { .. }
            | Msg::FaceClusterProgress { .. }
            | Msg::InferenceEngineReady { .. }
            | Msg::FaceClusteringDone(_)
            | Msg::FaceClustersBatchDone(_)
            | Msg::FaceClustersLoaded(_)
            | Msg::FaceCropsReady(_)
            | Msg::OpenPeopleView
            | Msg::RenameFaceCluster(_)
            | Msg::RenameFaceClusterInputChanged(_)
            | Msg::ConfirmRenameFaceCluster
            | Msg::MergeFaceClusters(_, _)
            | Msg::RemoveFileFromFaceCluster(_, _) => self.handle_extension_msg(msg),

            // — scanning & file watching —
            Msg::SyncPickFolder
            | Msg::SyncDialogDone(_)
            | Msg::SyncStart(_)
            | Msg::SyncComplete { .. }
            | Msg::RequestRemoveFolder(_)
            | Msg::CancelRemoveFolder
            | Msg::RemoveFolder(_)
            | Msg::FolderRemoved
            | Msg::SyncFolder(_)
            | Msg::FileWatcherEvent(_)
            | Msg::FlushFileEvents(_)
            | Msg::SyncXmpForSelection
            | Msg::SyncAppleTagsForSelection
            | Msg::MetadataImportPromptToggleXmp
            | Msg::MetadataImportPromptToggleApple
            | Msg::MetadataImportPromptToggleAll
            | Msg::MetadataImportPromptContinue
            | Msg::MetadataImportPromptCancel => self.handle_sync_msg(msg),

            Msg::RequestRemoveMissing(_)
            | Msg::ConfirmRemoveMissing
            | Msg::CancelRemoveMissing
            | Msg::LocateFile(_)
            | Msg::FileLocated { .. } => self.handle_missing_msg(msg),

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
            | Msg::AcceptAllInView
            | Msg::RejectAllInView
            | Msg::PendingCountsLoaded { .. }
            | Msg::PendingTotalLoaded(_)
            | Msg::SetSuggestionView(_)
            | Msg::PendingTagGroupsLoaded(_)
            | Msg::AcceptPendingTagGlobally(_)
            | Msg::RejectPendingTagGlobally(_)
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
            | Msg::HoverMenuTab(_)
            | Msg::CloseMenuDropdown
            | Msg::LoupeFullResLoaded { .. }
            | Msg::LoupePrefetchLoaded { .. }
            | Msg::SelectAll
            | Msg::DeselectAll
            | Msg::OpenCompare
            | Msg::CompareFullResLoaded { .. }
            | Msg::ShowInFinder(_)
            | Msg::SidebarScrolled(_) => self.handle_navigation_msg(msg),

            Msg::BgTaskDismissed(id) => {
                self.bg_tasks.retain(|t| t.id != id);
                Task::none()
            }

            Msg::ToggleTaskPanel => {
                self.task_panel_open = !self.task_panel_open;
                Task::none()
            }

            Msg::ExportSelectionToDialog(mode) => {
                let paths: Vec<String> = self
                    .grid_selected
                    .iter()
                    .filter_map(|id| self.files.iter().find(|f| &f.id == id))
                    .filter(|f| !f.is_orphaned)
                    .map(|f| f.path.clone())
                    .collect();
                if paths.is_empty() {
                    return Task::none();
                }
                self.context_menu = None;
                let title = match mode {
                    ExportMode::Copy => "Copy files to…",
                    ExportMode::Move => "Move files to…",
                };
                Task::perform(
                    async move {
                        let dest = rfd::AsyncFileDialog::new()
                            .set_title(title)
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_string_lossy().to_string());
                        (paths, dest, mode)
                    },
                    |(paths, dest, mode)| Msg::ExportDestPicked { paths, dest, mode },
                )
            }

            Msg::ExportDestPicked { paths, dest: None, .. } => {
                let _ = paths;
                Task::none()
            }

            Msg::ExportDestPicked { paths, dest: Some(dest), mode } => {
                let n = paths.len();
                let dest_name = std::path::Path::new(&dest)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&dest)
                    .to_string();
                let verb = match mode {
                    ExportMode::Copy => "Copying",
                    ExportMode::Move => "Moving",
                };
                let plural = if n == 1 { "" } else { "s" };
                let task_id = self.bg_push(format!("{verb} {n} file{plural} to \u{201c}{dest_name}\u{201d}\u{2026}"));
                self.task_panel_open = true;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            for src in &paths {
                                let filename = std::path::Path::new(src)
                                    .file_name()
                                    .ok_or_else(|| format!("invalid path: {src}"))?;
                                let dst = std::path::Path::new(&dest).join(filename);
                                match mode {
                                    ExportMode::Copy => {
                                        std::fs::copy(src, &dst)
                                            .map_err(|e| format!("copy {src}: {e}"))?;
                                    }
                                    ExportMode::Move => {
                                        std::fs::rename(src, &dst).or_else(|_| {
                                            std::fs::copy(src, &dst)
                                                .and_then(|_| std::fs::remove_file(src))
                                                .map_err(|e| std::io::Error::other(e.to_string()))
                                        }).map_err(|e| format!("move {src}: {e}"))?;
                                    }
                                }
                            }
                            Ok(())
                        })
                        .await
                        .unwrap_or_else(|e| Err(e.to_string()))
                    },
                    move |result| Msg::ExportDone { task_id, result },
                )
            }

            Msg::ExportDone { task_id, result } => {
                match result {
                    Ok(()) => self.bg_complete(task_id),
                    Err(e) => self.bg_fail(task_id, e),
                }
                Task::none()
            }

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
            | Msg::ToggleFilterPanel
            | Msg::FilterTagInputChanged(_)
            | Msg::AddFilterTag
            | Msg::RemoveFilterTag(_)
            | Msg::FilterDateFromChanged(_)
            | Msg::FilterDateToChanged(_)
            | Msg::ToggleFilterFileType(_)
            | Msg::ClearFilters => self.handle_filters(msg),

            // — settings panel —
            Msg::OpenSettings
            | Msg::SwitchSettingsTab(_)
            | Msg::CloseSettings
            | Msg::SettingsConfigChanged { .. }
            | Msg::SaveSettings
            | Msg::InstallExtensionPickFile
            | Msg::ExtensionPackagePicked(_)
            | Msg::ExtensionInstalled(_)
            | Msg::EngineInstalled(_)
            | Msg::ExtensionInstallFailed(_)
            | Msg::UninstallExtension(_)
            | Msg::SetPreferredExtension { .. }
            | Msg::ToggleAutoFaceCluster
            | Msg::ToggleInferenceCustom
            | Msg::InferenceUrlChanged(_)
            | Msg::InferencePortChanged(_)
            | Msg::FaceEpsChanged(_)
            | Msg::FaceMinPtsChanged(_)
            | Msg::ToggleImportXmpTags
            | Msg::ToggleImportAppleTags
            | Msg::ToggleAutoAdvanceOnFlag => self.handle_settings(msg),

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
                            self.filters.tags = q.tags.clone();
                            self.filters.date_from =
                                q.date_from.map(unix_to_date_str).unwrap_or_default();
                            self.filters.date_to =
                                q.date_to.map(unix_to_date_str).unwrap_or_default();
                            self.filters.exts = q.extensions.iter().cloned().collect();
                            self.search_text = q.text.clone().unwrap_or_default();
                            self.filters.has_location = q.has_location;
                            self.filters.show = true;
                        }
                    }
                }
                let going_to_suggestions = matches!(item, SidebarItem::Suggestions);
                self.selected_item = item;
                if matches!(self.view_mode, super::ViewMode::People) {
                    self.view_mode = super::ViewMode::Browse;
                }
                self.files.clear();
                self.file_ratings.clear();
                self.pending_counts_by_id.clear();
                self.scroll_y = 0.0;
                self.loupe.idx = 0;
                self.grid_selected.clear();
                self.drag.state = None;
                self.drag.ids.clear();
                self.filters.save_smart_input = None;
                self.detail.file_id = None;
                self.remove_from_album_pending = false;
                self.smart_album_dirty = false;
                if going_to_suggestions {
                    self.detail.show = true;
                    if matches!(self.suggestion_view, super::SuggestionView::Tag) {
                        return self.load_pending_tag_groups_task();
                    }
                }
                self.load_files_task()
            }

            Msg::FilesLoaded(files) => {
                self.files = files;
                self.enqueue_thumbnails();
                self.status = format!("{} photo(s)", self.files.len());
                let t1 = self.maybe_load_detail();
                let t2 = self.load_ratings_task();
                let t3 = if matches!(self.selected_item, SidebarItem::Suggestions) {
                    self.load_pending_counts_task()
                } else {
                    Task::none()
                };
                Task::batch([t1, t2, t3])
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
