use std::sync::{mpsc, Arc};

use iced::Task;
use isomfolio_core::app_paths::db_path;
use isomfolio_core::extension::discover_extensions;
use isomfolio_core::path_utils::CATALOG_EXT;

use super::LockUnwrap;
use super::super::{App, FilterState, DetailState, Msg, SidebarItem, ViewMode};

impl App {
    pub(super) fn handle_catalog_msg(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::CatalogReady => {
                self.start_thumbnail_pool();
                let sidebar_task = self.load_sidebar_task();
                let extension_task = Task::perform(
                    async move {
                        // The inference engine is launched on demand (HTTP), not here —
                        // just locate its manifest.
                        discover_extensions()
                            .into_iter()
                            .find(|m| m.capabilities.iter().any(|c| c == "inference_engine"))
                    },
                    Msg::ExtensionsDiscovered,
                );
                let face_task = if let Some(conn) = self.catalog.clone() {
                    Task::perform(
                        async move {
                            let g = conn.lock_unwrap();
                            g.get_face_cluster_summaries().unwrap_or_default()
                        },
                        Msg::FaceClustersLoaded,
                    )
                } else {
                    Task::none()
                };
                // Cache hygiene, off-thread: drop orphaned thumbnails for files no
                // longer catalogued.
                let maint_task = if let Some(conn) = self.catalog.clone() {
                    let dir = self.catalog_dir.clone();
                    Task::perform(
                        async move {
                            let _ = tokio::task::spawn_blocking(move || {
                                // Best-effort maintenance: a failed sweep must not
                                // surface to the user, but shouldn't vanish silently.
                                if let Err(e) = conn.lock_unwrap().sweep_caches(&dir) {
                                    eprintln!("cache sweep failed: {e}");
                                }
                            })
                            .await;
                        },
                        |_| Msg::NoOp,
                    )
                } else {
                    Task::none()
                };
                Task::batch([sidebar_task, extension_task, face_task, maint_task])
            }

            Msg::OpenCatalog(path) => {
                isomfolio_core::app_paths::save_recent_catalog(&path);
                self.watchers.clear();
                self.thumb_ctx.pool = None;
                let (new_tx, new_rx) = mpsc::sync_channel::<crate::app::ThumbnailEvent>(500);
                self.thumb_ctx.tx = new_tx;
                self.thumb_ctx.rx = Arc::new(std::sync::Mutex::new(Some(new_rx)));
                self.thumb_ctx.sub_id += 1;
                self.thumb_ctx.pending = 0;
                self.thumb_ctx.total = 0;
                self.thumb_ctx.start_at = None;
                self.thumb_ctx.done_gen += 1;
                self.files.clear();
                self.file_ratings.clear();
                self.thumbnails.clear();
                self.folders.clear();
                self.discovered_folders.clear();
                self.albums.clear();
                self.album_counts.clear();
                self.grid_selected.clear();
                self.selected_albums.clear();
                self.drag.state = None;
                self.drag.ids.clear();
                self.drag.album = None;
                self.hovered_shelf = None;
                self.search_debounce_id += 1;
                self.search_text.clear();
                self.filters = FilterState::default();
                self.detail = DetailState::default();
                self.selected_item = SidebarItem::AllFiles;
                self.scroll_y = 0.0;
                self.loupe.idx = 0;
                self.view_mode = ViewMode::Browse;
                self.loupe.full_res = None;
                self.loupe.prefetch.clear();
                self.album_pending_delete = None;
                self.folder_pending_remove = None;
                self.welcome.selected_recent_catalog = Some(path.clone());
                self.welcome.show_new_catalog_modal = false;
                self.welcome.new_catalog_dir = None;
                self.welcome.new_catalog_name.clear();
                isomfolio_core::app_paths::ensure_directories(&path);
                self.catalog = isomfolio_core::Catalog::open(&db_path(&path))
                    .ok()
                    .map(|c| std::sync::Arc::new(std::sync::Mutex::new(c)));
                self.status = if self.catalog.is_none() {
                    "Error: could not open database — check permissions".to_string()
                } else {
                    String::new()
                };
                self.catalog_dir = path;
                self.pending_restore = isomfolio_core::app_paths::read_last_session()
                    .filter(|s| s.catalog_path == self.catalog_dir)
                    .and_then(|s| s.last_selected)
                    .and_then(|t| crate::app::SidebarItem::from_token(&t));
                self.welcome.show = false;
                self.tag_browser = None;
                self.welcome.recent_catalogs = isomfolio_core::app_paths::read_recent_catalogs();
                Task::batch([Task::done(Msg::CatalogReady), App::resize_to_main()])
            }

            Msg::PickOpenCatalog => {
                self.open_menu = None;
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Open Catalog")
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    |opt| match opt {
                        Some(path) => Msg::OpenCatalogPicked(path),
                        None => Msg::NoOp,
                    },
                )
            }

            Msg::OpenCatalogPicked(path) => {
                if !is_catalog_dir(&path) {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    self.status = format!("\"{}\" is not a valid catalog", name);
                    return Task::none();
                }
                Task::done(Msg::OpenCatalog(path.to_string_lossy().into_owned()))
            }

            Msg::SelectRecentCatalog(path) => {
                self.welcome.selected_recent_catalog = Some(path);
                Task::none()
            }

            Msg::OpenSelectedRecentCatalog => {
                let Some(path) = self.welcome.selected_recent_catalog.clone() else {
                    return Task::none();
                };
                Task::done(Msg::OpenCatalog(path))
            }

            Msg::ShowNewCatalogModal => {
                self.open_menu = None;
                if self.welcome.show {
                    self.welcome.recent_catalogs = isomfolio_core::app_paths::read_recent_catalogs();
                }
                self.welcome.show_new_catalog_modal = true;
                self.welcome.new_catalog_dir = None;
                self.welcome.new_catalog_name.clear();
                iced::widget::operation::focus(crate::app::input_ids::new_catalog())
            }

            Msg::HideNewCatalogModal => {
                self.welcome.show_new_catalog_modal = false;
                self.welcome.new_catalog_dir = None;
                self.welcome.new_catalog_name.clear();
                Task::none()
            }

            Msg::PickNewCatalogDir => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose location for new catalog")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                |opt| match opt {
                    Some(dir) => Msg::NewCatalogDirPicked(dir),
                    None => Msg::NoOp,
                },
            ),

            Msg::NewCatalogDirPicked(dir) => {
                self.welcome.new_catalog_dir = Some(dir);
                Task::none()
            }

            Msg::NewCatalogNameChanged(s) => {
                self.welcome.new_catalog_name = s;
                Task::none()
            }

            Msg::ConfirmNewCatalog => {
                let Some(dir) = self.welcome.new_catalog_dir.clone() else {
                    return Task::none();
                };
                let name = self.welcome.new_catalog_name.trim().to_string();
                if name.is_empty() {
                    return Task::none();
                }
                self.welcome.show_new_catalog_modal = false;
                Task::perform(
                    async move {
                        isomfolio_core::app_paths::create_catalog(&dir.to_string_lossy(), &name)
                            .map_err(|e| e.to_string())
                    },
                    |result| match result {
                        Ok(path) => Msg::OpenCatalog(path),
                        Err(e) => Msg::DbError(e),
                    },
                )
            }

            other => {
                debug_assert!(false, "handle_catalog_msg received misrouted message: {other:?}");
                Task::none()
            }
        }
    }
}

fn is_catalog_dir(path: &std::path::Path) -> bool {
    path.extension().map_or(false, |ext| ext == CATALOG_EXT) && path.join("catalog.db").exists()
}
