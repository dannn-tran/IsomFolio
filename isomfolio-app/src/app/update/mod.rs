mod albums;
mod catalog;
mod criteria;
mod detail;
mod drag_drop;
mod extensions;
mod loupe_load;
mod navigation;
mod pointer;
mod groups;
mod sync;
mod settings;
mod stacking;
mod tag_browser;

use iced::Task;
use isomfolio_core::models::{Album, SearchQuery, ThumbnailState};

use super::{
    unix_to_date_str, AlbumKind, App, CopyEntry, ExportMode, Msg, SidebarItem,
};

pub(super) use super::LockUnwrap;

impl App {
    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        // Any context-menu leaf action closes the menu first, then runs. Single
        // chokepoint so no individual handler has to remember to dismiss it.
        if let Msg::MenuAction(inner) = msg {
            self.context_menu = None;
            return self.update(*inner);
        }
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

            // — inference engine & face clustering —
            Msg::ExtensionsDiscovered(_)
            | Msg::RunFaceClustering { .. }
            | Msg::FaceClusterProgress { .. }
            | Msg::InferenceEngineReady { .. }
            | Msg::FaceClusteringDone(_)
            | Msg::FaceClustersLoaded(_)
            | Msg::FaceCropsReady(_)
            | Msg::OpenPeopleView
            | Msg::RenameFaceCluster(_)
            | Msg::RenameFaceClusterInputChanged(_)
            | Msg::ConfirmRenameFaceCluster
            | Msg::MergeFaceClusters(_, _)
            | Msg::FaceClusterCardClicked(_)
            | Msg::ClearFaceSelection
            | Msg::BatchFaceNameInputChanged(_)
            | Msg::ConfirmBatchFaceNameMerge => self.handle_extension_msg(msg),

            // — scanning & file watching —
            Msg::SyncPickFolder
            | Msg::SyncPickFolderAt(_)
            | Msg::SyncDialogDone(_)
            | Msg::SyncStart { .. }
            | Msg::AddFolderPromptToggleRecursive
            | Msg::AddFolderConfirm
            | Msg::AddFolderCancel
            | Msg::ToggleFolderExpanded(_)
            | Msg::SyncComplete { .. }
            | Msg::RequestRemoveFolder(_)
            | Msg::CancelRemoveFolder
            | Msg::RemoveFolder(_)
            | Msg::FolderRemoved
            | Msg::SyncFolder(_)
            | Msg::SyncSelectedFolder
            | Msg::FileWatcherEvent(_)
            | Msg::FlushFileEvents(_)
            | Msg::SyncXmpForSelection
            | Msg::SyncAppleTagsForSelection => self.handle_sync_msg(msg),

            Msg::RequestRemoveMissing(_)
            | Msg::ConfirmRemoveMissing
            | Msg::CancelRemoveMissing
            | Msg::DeleteSelection
            | Msg::RestoreSelection
            | Msg::RequestDeleteRejects
            | Msg::ConfirmDeleteRejects
            | Msg::CancelDeleteRejects
            | Msg::SelectionDeleted
            | Msg::RequestPurgeSelected
            | Msg::RequestPurgeAll
            | Msg::ConfirmPurge
            | Msg::CancelPurge
            | Msg::Purged(_)
            | Msg::LocateFile(_)
            | Msg::FileLocated { .. } => self.handle_missing_msg(msg),

            // — detail panel, tags, ratings, flags, undo —
            Msg::ToggleDetail
            | Msg::DetailLoaded { .. }
            | Msg::DetailFieldChanged(_, _)
            | Msg::SaveDetailField(_)
            | Msg::BatchDetailLoaded { .. }
            | Msg::BatchTagsChanged
            | Msg::DetailTagInputChanged(_)
            | Msg::AddDetailTag
            | Msg::AddDetailTagDirect(_)
            | Msg::RemoveDetailTag(_)
            | Msg::AllTagsLoaded(_)
            | Msg::TagsSavedResult(_, _)
            | Msg::RepeatLastTag
            | Msg::FocusTagInput
            | Msg::SetDetailRating(_)
            | Msg::SetFlag(_)
            | Msg::FlagsApplied
            | Msg::SetRating(_)
            | Msg::RatingsApplied
            | Msg::SetColorLabel(_)
            | Msg::LabelsApplied
            | Msg::FileSideDataLoaded { .. }
            | Msg::ToggleHideRejects
            | Msg::ToggleFlagFilter(_)
            | Msg::SetRatingFilter(_)
            | Msg::SetRatingCmp(_)
            | Msg::SetLocationFilter(_)
            | Msg::Undo
            | Msg::Redo
            | Msg::UndoApplied => self.handle_detail_msg(msg),

            // — navigation, mouse, loupe, compare, context menu —
            Msg::TileSizeUp
            | Msg::TileSizeDown
            | Msg::SetTileSize(_)
            | Msg::WindowResized(_)
            | Msg::Navigate { .. }
            | Msg::NavigateExtend { .. }
            | Msg::ToggleFullscreen
            | Msg::EscapePressed
            | Msg::Scrolled { .. }
            | Msg::ToggleShortcutHelp
            | Msg::OpenMenuDropdown(_)
            | Msg::HoverMenuTab(_)
            | Msg::CloseMenuDropdown
            | Msg::SelectAll
            | Msg::DeselectAll
            | Msg::ShowInFinder(_)
            | Msg::SidebarScrolled(_) => self.handle_navigation_msg(msg),

            // — pointer / drag interaction + context menus —
            Msg::SidebarResizeStart
            | Msg::ListColResizeStart(_)
            | Msg::MouseMoved(_)
            | Msg::MouseRightClicked
            | Msg::MousePressed
            | Msg::MouseReleased
            | Msg::ModifiersChanged(_)
            | Msg::OpenFaceClusterMenu(_)
            | Msg::OpenAlbumsAddMenu
            | Msg::ToggleAddToAlbumSubmenu
            | Msg::HoverSidebarEntityStart(_)
            | Msg::HoverSidebarEntityEnd(_)
            | Msg::OpenSidebarEntityMenu(_)
            | Msg::AlbumPressed(_)
            | Msg::HoverDrop(_) => self.handle_pointer_msg(msg),

            // — loupe / preview / compare —
            Msg::OpenLoupe
            | Msg::LoupeZoomChanged { .. }
            | Msg::LoupeZoomBy(_)
            | Msg::LoupeZoomReset
            | Msg::LoupeZoomActual
            | Msg::LoupeGeometry { .. }
            | Msg::ToggleLoupeZoomLock
            | Msg::LoupeJumpTo(_)
            | Msg::TogglePreview
            | Msg::SetBrowseLayout(_)
            | Msg::LoupeFullResLoaded { .. }
            | Msg::LoupeFullResFailed { .. }
            | Msg::OpenPrivacySettings
            | Msg::LoupeHiresLoaded { .. }
            | Msg::LoupePrefetchLoaded { .. }
            | Msg::OpenCompare
            | Msg::CompareFullResLoaded { .. }
            | Msg::CompareZoomChanged { .. } => self.handle_loupe_msg(msg),

            // — content-based stacking —
            Msg::RunStacking
            | Msg::RestackNow
            | Msg::StacksUpdated
            | Msg::StackStatsLoaded(_)
            | Msg::StackKeepOnly(_)
            | Msg::StackRejectAll(_)
            | Msg::ToggleStackExpanded(_)
            | Msg::StackFlagsApplied { .. } => self.handle_stacking_msg(msg),

            Msg::BgTaskDismissed(id) => {
                self.bg_tasks.retain(|t| t.id != id);
                Task::none()
            }

            Msg::ToggleTaskPanel => {
                self.task_panel_open = !self.task_panel_open;
                Task::none()
            }

            Msg::WriteXmpSidecars => {
                let items: Vec<(String, String)> = self
                    .grid_selected
                    .iter()
                    .filter_map(|id| self.files.iter().find(|f| &f.id == id))
                    .map(|f| (f.id.clone(), f.disk_path()))
                    .collect();
                if items.is_empty() {
                    return Task::none();
                }
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                self.status = format!("Writing {} XMP sidecar(s)…", items.len());
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let cat = conn.lock_unwrap();
                            let (mut ok, mut failed) = (0usize, 0usize);
                            for (id, path) in &items {
                                let sidecar = std::path::Path::new(path).with_extension("xmp");
                                // Merge onto any existing sidecar so unmanaged
                                // fields/namespaces are preserved.
                                let existing = std::fs::read_to_string(&sidecar).ok();
                                match cat.xmp_sidecar_for(id, existing.as_deref()) {
                                    Ok(xml) if std::fs::write(&sidecar, &xml).is_ok() => ok += 1,
                                    _ => failed += 1,
                                }
                            }
                            (ok, failed)
                        })
                        .await
                        .unwrap_or((0, 0))
                    },
                    |(count, failed)| Msg::SidecarsWritten { count, failed },
                )
            }

            Msg::SidecarsWritten { count, failed } => {
                self.status = if failed > 0 {
                    format!("Wrote {count} XMP sidecar(s), {failed} failed")
                } else {
                    format!("Wrote {count} XMP sidecar(s)")
                };
                Task::none()
            }

            Msg::ExportMetadata => {
                let ids: Vec<String> = if self.grid_selected.is_empty() {
                    self.files.iter().map(|f| f.id.clone()).collect()
                } else {
                    self.grid_selected.iter().cloned().collect()
                };
                if ids.is_empty() {
                    return Task::none();
                }
                Task::perform(
                    async move {
                        let dest = rfd::AsyncFileDialog::new()
                            .set_title("Export metadata as CSV")
                            .set_file_name("metadata.csv")
                            .save_file()
                            .await
                            .map(|h| h.path().to_string_lossy().to_string());
                        (ids, dest)
                    },
                    |(ids, dest)| Msg::ExportMetadataDest { ids, dest },
                )
            }

            Msg::ExportMetadataDest { dest: None, .. } => Task::none(),

            Msg::ExportMetadataDest { ids, dest: Some(dest) } => {
                let Some(conn) = self.catalog.clone() else { return Task::none() };
                let count = ids.len();
                self.status = format!("Exported {count} row(s) to CSV");
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let cat = conn.lock_unwrap();
                            if let Ok(csv) = cat.export_metadata_csv(&ids) {
                                let _ = std::fs::write(&dest, csv);
                            }
                        })
                        .await
                        .ok();
                        count
                    },
                    |_| Msg::MetadataExported,
                )
            }

            Msg::MetadataExported => Task::none(),

            Msg::ExportSelectionToDialog(mode) => {
                // Loose photos copy straight into the chosen folder (rel empty).
                let entries: Vec<CopyEntry> = self
                    .grid_selected
                    .iter()
                    .filter_map(|id| self.files.iter().find(|f| &f.id == id))
                    .filter(|f| !f.is_orphaned)
                    .map(|f| CopyEntry { rel: Vec::new(), src: export_source_path(f) })
                    .collect();
                if entries.is_empty() {
                    return Task::none();
                }
                self.context_menu = None;
                let title = match mode {
                    ExportMode::Copy => "Copy files to…",
                };
                Task::perform(
                    async move {
                        let dest = rfd::AsyncFileDialog::new()
                            .set_title(title)
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_string_lossy().to_string());
                        (entries, dest, mode)
                    },
                    |(entries, dest, mode)| Msg::ExportDestPicked { entries, dest, mode },
                )
            }

            Msg::ExportAlbumToDialog(album_id) => {
                let Some(catalog) = self.catalog.clone() else { return Task::none() };
                self.context_menu = None;
                let Some(album) = self.albums.iter().find(|a| a.id == album_id).cloned() else {
                    return Task::none();
                };
                // Each album becomes its own sub-folder named after the album.
                let folder = isomfolio_core::fileops::sanitize_component(&album.name);
                Task::perform(
                    async move {
                        let entries: Vec<CopyEntry> = {
                            let cat = catalog.lock_unwrap();
                            album_source_paths(&cat, &album)
                                .into_iter()
                                .map(|src| CopyEntry { rel: vec![folder.clone()], src })
                                .collect()
                        };
                        let dest = if entries.is_empty() {
                            None
                        } else {
                            rfd::AsyncFileDialog::new()
                                .set_title("Copy album to…")
                                .pick_folder()
                                .await
                                .map(|h| h.path().to_string_lossy().to_string())
                        };
                        (entries, dest)
                    },
                    |(entries, dest)| Msg::ExportDestPicked { entries, dest, mode: ExportMode::Copy },
                )
            }

            Msg::ExportGroupToDialog(group_id) => {
                let Some(catalog) = self.catalog.clone() else { return Task::none() };
                self.context_menu = None;
                if !self.groups.iter().any(|g| g.id == group_id) {
                    return Task::none();
                }
                // Mirror the group's whole subtree on disk: every album in this
                // group *or any nested sub-group* becomes
                // <group>/<sub-group>/…/<album>/…, recursing so nested albums
                // aren't dropped. The rel prefix is the chain of group names from
                // the exported root down to each album's immediate group.
                let albums_with_rel: Vec<(Album, Vec<String>)> = self
                    .albums
                    .iter()
                    .filter_map(|a| {
                        let gid = a.group_id.as_deref()?;
                        let mut rel = group_name_chain(&self.groups, &group_id, gid)?;
                        rel.push(isomfolio_core::fileops::sanitize_component(&a.name));
                        Some((a.clone(), rel))
                    })
                    .collect();
                Task::perform(
                    async move {
                        let entries: Vec<CopyEntry> = {
                            let cat = catalog.lock_unwrap();
                            albums_with_rel
                                .iter()
                                .flat_map(|(album, rel)| {
                                    let rel = rel.clone();
                                    album_source_paths(&cat, album)
                                        .into_iter()
                                        .map(move |src| CopyEntry { rel: rel.clone(), src })
                                })
                                .collect()
                        };
                        let dest = if entries.is_empty() {
                            None
                        } else {
                            rfd::AsyncFileDialog::new()
                                .set_title("Copy group to…")
                                .pick_folder()
                                .await
                                .map(|h| h.path().to_string_lossy().to_string())
                        };
                        (entries, dest)
                    },
                    |(entries, dest)| Msg::ExportDestPicked { entries, dest, mode: ExportMode::Copy },
                )
            }

            Msg::ExportDestPicked { entries, dest: None, .. } => {
                let _ = entries;
                Task::none()
            }

            Msg::ExportDestPicked { entries, dest: Some(dest), mode } => {
                let n = entries.len();
                let dest_name = std::path::Path::new(&dest)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&dest)
                    .to_string();
                let verb = match mode {
                    ExportMode::Copy => "Copying",
                };
                let plural = if n == 1 { "" } else { "s" };
                let task_id = self.bg_push(format!("{verb} {n} file{plural} to \u{201c}{dest_name}\u{201d}\u{2026}"));
                self.task_panel_open = true;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let root = std::path::Path::new(&dest);
                            for entry in &entries {
                                let mut dir = root.to_path_buf();
                                for component in &entry.rel {
                                    dir.push(component);
                                }
                                match mode {
                                    ExportMode::Copy => {
                                        isomfolio_core::fileops::copy_into_dir(
                                            std::path::Path::new(&entry.src),
                                            &dir,
                                        )
                                        .map_err(|e| format!("copy {}: {e}", entry.src))?;
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
            | Msg::SetTargetAlbum(_)
            | Msg::AddSelectionToTargetAlbum
            | Msg::DuplicateAlbum(_)
            | Msg::StartCreateAlbum
            | Msg::CreateAlbumInputChanged(_)
            | Msg::ConfirmCreateAlbum
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
            | Msg::DeleteKeyPressed
            | Msg::SaveAsSmartAlbum
            | Msg::SmartAlbumNameChanged(_)
            | Msg::ConfirmSmartAlbum
            | Msg::SmartAlbumUpdated
            | Msg::UpdateSmartAlbum => self.handle_album_msg(msg),

            // — groups —
            Msg::StartCreateGroup
            | Msg::CreateGroupInputChanged(_)
            | Msg::ConfirmCreateGroup
            | Msg::GroupCreated
            | Msg::StartRenameGroup(_)
            | Msg::RenameGroupInputChanged(_)
            | Msg::ConfirmRenameGroup
            | Msg::GroupRenamed
            | Msg::RequestDeleteGroup(_)
            | Msg::CancelDeleteGroup
            | Msg::DeleteGroup(_)
            | Msg::GroupDeleted
            | Msg::ToggleGroupCollapsed(_)
            | Msg::GroupHeaderPressed(_)
            | Msg::OpenGroupMenu(_)
            | Msg::MoveAlbumsToGroup { .. }
            | Msg::StartCreateGroupFor(_)
            | Msg::StartCreateGroupIn(_)
            | Msg::StartCreateAlbumIn(_)
            | Msg::SelectGroupAlbums(_)
            | Msg::MoveGroupToParent { .. }
            | Msg::GroupMoved
            | Msg::AlbumMovedToGroup => self.handle_group_msg(msg),

            // — search & filter criteria —
            Msg::SortDirToggle
            | Msg::SetSortField(_)
            | Msg::SetGridLayout(_)
            | Msg::SearchChanged(_)
            | Msg::ToggleFilterPanel
            | Msg::FilterTagInputChanged(_)
            | Msg::AddFilterTag
            | Msg::RemoveFilterTag(_)
            | Msg::ToggleFilterTagNegate(_)
            | Msg::SetTagMatch(_)
            | Msg::FilterDateFromChanged(_)
            | Msg::FilterDateToChanged(_)
            | Msg::SetDatePreset(_)
            | Msg::SetPersonFilter(_)
            | Msg::SetAddedWithinFilter(_)
            | Msg::SetCameraFilter(_)
            | Msg::SetColorFilter(_)
            | Msg::ToggleFilterFileType(_)
            | Msg::ToggleCollapseBursts
            | Msg::ClearFilters => self.handle_filters(msg),

            // — settings panel —
            Msg::OpenSettings
            | Msg::SwitchSettingsTab(_)
            | Msg::CloseSettings
            | Msg::SettingsConfigChanged { .. }
            | Msg::SaveSettings
            | Msg::InstallExtensionPickFile
            | Msg::ExtensionPackagePicked(_)
            | Msg::EngineInstalled(_)
            | Msg::ExtensionInstallFailed(_)
            | Msg::UninstallExtension(_)
            | Msg::ToggleAutoFaceCluster
            | Msg::ToggleInferenceCustom
            | Msg::InferenceUrlChanged(_)
            | Msg::InferencePortChanged(_)
            | Msg::FaceEpsChanged(_)
            | Msg::FaceMinPtsChanged(_)
            | Msg::ToggleImportXmpTags
            | Msg::ToggleImportAppleTags
            | Msg::ToggleAutoAdvanceOnCull
            | Msg::ToggleAutoStack
            | Msg::StackThresholdChanged(_)
            | Msg::StackWindowChanged(_) => self.handle_settings(msg),

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
                // Navigating away abandons any album multi-selection (the highlight
                // would otherwise linger over an unrelated view).
                if !matches!(item, SidebarItem::Album(_)) {
                    self.selected_albums.clear();
                }
                if let SidebarItem::Album(ref id) = item {
                    if let Some(album) = self.albums.iter().find(|a| &a.id == id) {
                        if let AlbumKind::Smart(ref q) = album.kind {
                            self.filters.tags = q.tags.clone();
                            self.filters.tag_match = q.tag_match;
                            self.filters.exclude_tags = q.exclude_tags.clone();
                            self.filters.date_from =
                                q.date_from.map(unix_to_date_str).unwrap_or_default();
                            self.filters.date_to =
                                q.date_to.map(unix_to_date_str).unwrap_or_default();
                            self.filters.exts = q.extensions.iter().cloned().collect();
                            self.search_text = q.text.clone().unwrap_or_default();
                            self.filters.has_location = q.has_location;
                            self.filters.rating = q.rating;
                            self.filters.flags = q.flags;
                            self.filters.person = q.person_cluster.clone();
                            self.filters.camera = q.camera_model.clone();
                            self.filters.color = q.color_label.clone();
                            self.filters.added_within_days = q.added_within_days;
                            self.collapsed_sections.remove(&crate::app::SidebarSection::Filters);
                        }
                    }
                }
                // Remember where we were in the outgoing view, then look up a
                // saved position for the one we're switching to so returning to
                // a folder/album lands where we left off instead of the top.
                if let Some(idx) = self.anchor_idx {
                    self.saved_positions.insert(self.selected_item.to_token(), idx);
                }
                let restore_idx = self.saved_positions.get(&item.to_token()).copied();
                self.selected_item = item;
                if matches!(self.view_mode, super::ViewMode::People) {
                    self.view_mode = super::ViewMode::Browse;
                }
                self.files.clear();
                self.file_ratings.clear();
                self.file_labels.clear();
                self.file_burst_sizes.clear();
                self.scroll_y = 0.0;
                self.loupe.idx = 0;
                self.anchor_idx = None;
                self.select_lead = None;
                self.pending_restore_idx = restore_idx;
                self.grid_selected.clear();
                self.selection_base.clear();
                self.drag.current = None;
                self.drag.hover = None;
                self.filters.save_smart_input = None;
                self.detail.file_id = None;
                self.remove_from_album_pending = false;
                self.smart_album_dirty = false;
                self.save_session();
                self.load_files_task()
            }

            Msg::FilesLoaded(files) => {
                self.files = files;
                self.enqueue_thumbnails();
                self.status = format!("{} photo(s)", self.files.len());
                // Undo/redo re-centring (id-based): jump the view back to the photo(s)
                // the undone/redone edit touched, *before* anything clamps. In loupe
                // this restores `loupe.idx` (so undoing an auto-advanced edit returns
                // to the image); in grid it re-selects them. Ids no longer present
                // (e.g. a re-applied delete) leave the view to fall through to the
                // clamp below. Wins over `pending_restore_idx`.
                let mut focus_scroll: Option<Task<Msg>> = None;
                let focused = if let Some(ids) = self.pending_focus_files.take() {
                    if let Some(idx) = self.files.iter().position(|f| ids.contains(&f.id)) {
                        if matches!(self.view_mode, super::ViewMode::Loupe) {
                            self.loupe.idx = idx;
                        } else {
                            let present: Vec<String> = self
                                .files
                                .iter()
                                .filter(|f| ids.contains(&f.id))
                                .map(|f| f.id.clone())
                                .collect();
                            self.anchor_idx = Some(idx);
                            self.select_lead = Some(idx);
                            self.grid_selected.clear();
                            self.selection_base.clear();
                            for id in present {
                                self.grid_selected.insert(id);
                            }
                            focus_scroll = Some(self.scroll_to_index(idx));
                        }
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                // The loupe full-res cache is keyed only by index, so any list
                // mutation (delete, filter, re-sort) leaves it pointing at a photo
                // that may no longer live at that slot. Re-sync: clamp the index,
                // drop the stale image, reload — or fall back to the grid if the
                // view is now empty. Covers the loupe-delete "advance to next" case.
                let loupe_resync = if matches!(self.view_mode, super::ViewMode::Loupe) {
                    if self.files.is_empty() {
                        self.view_mode = super::ViewMode::Browse;
                        self.loupe.full_res = None;
                        self.loupe.prefetch.clear();
                        None
                    } else {
                        self.loupe.idx = self.loupe.idx.min(self.files.len() - 1);
                        self.loupe.full_res = None;
                        self.loupe.prefetch.clear();
                        Some(self.load_loupe_full_res())
                    }
                } else {
                    None
                };
                let restore = (!focused).then(|| self.pending_restore_idx.take()).flatten().and_then(|idx| {
                    // Clamp instead of dropping: deleting the last photo (or returning
                    // to a now-shorter view) should still land on the neighbour that
                    // slid into place — matches Finder/Lightroom/Photos selecting the
                    // next item, or the previous one when the last was removed.
                    (!self.files.is_empty()).then(|| {
                        let idx = idx.min(self.files.len() - 1);
                        self.anchor_idx = Some(idx);
                        self.select_lead = Some(idx);
                        self.grid_selected.clear();
                        self.selection_base.clear();
                        if let Some(f) = self.files.get(idx) {
                            self.grid_selected.insert(f.id.clone());
                        }
                        self.scroll_to_index(idx)
                    })
                });
                let t1 = self.maybe_load_detail();
                let t2 = self.load_file_side_data_task();
                let mut tasks = vec![t1, t2];
                if let Some(scroll) = restore {
                    tasks.push(scroll);
                }
                if let Some(scroll) = focus_scroll {
                    tasks.push(scroll);
                }
                if let Some(loupe) = loupe_resync {
                    tasks.push(loupe);
                }
                Task::batch(tasks)
            }

            Msg::ImportBatchesLoaded(batches) => {
                self.import_batches = batches;
                Task::none()
            }

            Msg::ToggleShowAllImports => {
                self.show_all_imports = !self.show_all_imports;
                Task::none()
            }

            Msg::ToggleSidebarSection(section) => {
                if !self.collapsed_sections.remove(&section) {
                    self.collapsed_sections.insert(section);
                }
                Task::none()
            }

            Msg::PruneCompletedTasks => {
                self.completed_tasks
                    .retain(|t| t.at.elapsed() < super::COMPLETED_TTL);
                Task::none()
            }

            Msg::RecheckOfflineRoots => {
                // Stat the roots off-thread (a dead mount can block) and report
                // which are offline; the UI thread only diffs the result. Stat the
                // *real-cased* path (folded won't resolve on case-sensitive
                // volumes), but key the result by the folded `path` so it matches
                // `file.folder` in `is_offline_path`.
                let pairs: Vec<(String, String)> = self
                    .library_roots
                    .iter()
                    .map(|r| (r.path.clone(), root_disk_path(r)))
                    .collect();
                if pairs.is_empty() {
                    return Task::none();
                }
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            pairs
                                .into_iter()
                                .filter(|(_, disk)| !std::path::Path::new(disk).is_dir())
                                .map(|(key, _)| key)
                                .collect::<std::collections::HashSet<String>>()
                        })
                        .await
                        .unwrap_or_default()
                    },
                    Msg::OfflineRootsChecked,
                )
            }

            Msg::OfflineRootsChecked(offline) => {
                if offline == self.offline_roots {
                    return Task::none();
                }
                // A drive came back or dropped — reload the sidebar (recomputes
                // offline state, re-establishes watchers for now-reachable roots).
                // Grid/sidebar "Offline" markers derive from `offline_roots`, so
                // they refresh on the reload's view pass.
                self.load_sidebar_task()
            }

            Msg::SidebarLoaded { folders, folder_tree, library_roots, offline_roots, cameras, albums, groups, album_counts, deleted_count, import_batches } => {
                self.folders = folders;
                self.folder_tree = folder_tree;
                self.library_roots = library_roots;
                self.offline_roots = offline_roots;
                self.cameras = cameras;
                self.albums = albums;
                self.groups = groups;
                // Drop collapse state for groups that no longer exist.
                self.collapsed_groups.retain(|id| self.groups.iter().any(|s| &s.id == id));
                self.album_counts = album_counts;
                self.deleted_count = deleted_count;
                self.import_batches = import_batches;
                if let Some(target) = self.expand_under_path.take() {
                    for p in isomfolio_core::folder_tree::expand_paths_for(&self.folder_tree, &target) {
                        self.expanded_folders.insert(p);
                    }
                }
                self.start_watchers_for_folders();
                let restore = self.pending_restore.take().filter(|item| match item {
                    SidebarItem::AllFiles | SidebarItem::Deleted => true,
                    SidebarItem::Folder(p) => self.folders.iter().any(|(fp, _, _)| fp == p),
                    SidebarItem::Album(id) => self.albums.iter().any(|a| &a.id == id),
                    SidebarItem::Import(id) => self.import_batches.iter().any(|b| b.id == *id),
                });
                if let Some(item) = restore {
                    Task::done(Msg::SidebarItemClicked(item))
                } else if let Some(id) = self.pending_album_select.take() {
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
            // The JPEG is on disk; mark Ready and let the renderer decode it by
            // path on demand (no in-app decode/handle cache).
            Msg::ThumbnailCompleted { file_id, path } => {
                self.thumbnails.insert(file_id, ThumbnailState::Ready(path));
                self.thumb_settled()
            }

            Msg::ThumbnailFailed { file_id } => {
                self.thumbnails.insert(file_id, ThumbnailState::Failed(0));
                self.thumb_settled()
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
                self.selection_base.clear();
                self.anchor_idx = None;
                self.select_lead = None;
                self.load_files_task()
            }

            Msg::DbError(e) => {
                self.status = format!("Error: {e}");
                Task::none()
            }

            Msg::NoOp => Task::none(),

            // Unwrapped above, before this match.
            Msg::MenuAction(_) => unreachable!(),
        }
    }

    pub(super) fn mark_smart_dirty(&mut self) {
        if self.current_album_is_smart() {
            self.smart_album_dirty = true;
        }
    }

    /// Decrement the in-flight thumbnail count after one settles (ready or
    /// failed). When the batch drains, schedule the 2-second lingering-progress
    /// clear so the progress chip doesn't vanish the instant the last tile lands.
    fn thumb_settled(&mut self) -> Task<Msg> {
        self.thumb_ctx.pending = self.thumb_ctx.pending.saturating_sub(1);
        if self.thumb_ctx.pending == 0 && self.thumb_ctx.total > 0 {
            let gen = self.thumb_ctx.done_gen;
            let clear = Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        gen
                    })
                    .await
                    .unwrap_or(gen)
                },
                Msg::ClearThumbnailProgress,
            );
            // Newly-cached thumbnails are now hashable / embeddable — refresh.
            let mut tasks = vec![clear];
            if self.app_settings.auto_stack {
                tasks.push(Task::done(Msg::RunStacking));
            }
            Task::batch(tasks)
        } else {
            Task::none()
        }
    }

    /// File ids an edit (flag/rating/colour/delete) acts on: in the loupe that is
    /// the single photo on display (`loupe.idx`), otherwise the grid selection.
    /// The loupe target is *not* the grid selection — that may lag behind loupe
    /// navigation — so callers must use this rather than reading `grid_selected`.
    pub(super) fn selection_target_ids(&self) -> Vec<String> {
        if matches!(self.view_mode, super::ViewMode::Loupe) {
            self.files
                .get(self.loupe.idx)
                .map(|f| vec![f.id.clone()])
                .unwrap_or_default()
        } else {
            self.grid_selected.iter().cloned().collect()
        }
    }
}

/// Real-cased on-disk path for export — see [`AssetFile::disk_path`]. Copying
/// from the folded `path` would both fail on case-sensitive volumes and produce
/// a lower-cased destination filename.
fn export_source_path(f: &isomfolio_core::models::AssetFile) -> String {
    f.disk_path()
}

/// Real-cased disk paths of every present (non-orphaned) file in an album —
/// smart albums resolve their query, manual albums list their membership.
/// Sanitised group names from the exported root group down to `gid` (inclusive),
/// or `None` when `gid` is not `root` or one of its descendants. Used to mirror a
/// group's nesting as on-disk sub-folders when copying a group to a folder.
fn group_name_chain(
    groups: &[isomfolio_core::models::Group],
    root: &str,
    gid: &str,
) -> Option<Vec<String>> {
    let mut chain = Vec::new();
    let mut cur = Some(gid.to_string());
    loop {
        let c = cur?;
        let g = groups.iter().find(|g| g.id == c)?;
        chain.push(isomfolio_core::fileops::sanitize_component(&g.name));
        if c == root {
            break;
        }
        cur = g.parent_id.clone();
    }
    chain.reverse();
    Some(chain)
}

fn album_source_paths(cat: &isomfolio_core::Catalog, album: &Album) -> Vec<String> {
    let files = match &album.kind {
        AlbumKind::Smart(q) => cat.search(q).unwrap_or_default(),
        _ => cat
            .search_manual_album(&album.id, &SearchQuery::default())
            .unwrap_or_default(),
    };
    files.iter().filter(|f| !f.is_orphaned).map(export_source_path).collect()
}

/// Real-cased path of a library root for disk stat (`is_dir`). Like
/// [`AssetFile::disk_path`], the folded `path` only resolves on case-insensitive
/// volumes; use `path_display` (falls back to `path` when unset).
fn root_disk_path(r: &isomfolio_core::LibraryRoot) -> String {
    if r.path_display.is_empty() {
        r.path.clone()
    } else {
        r.path_display.clone()
    }
}
