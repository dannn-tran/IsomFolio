mod types;
mod update;

pub use types::*;

use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

use iced::{event, keyboard, mouse, Event, Point, Size, Subscription, Task};

use isomfolio_core::addon::AddonProcess;
use isomfolio_core::app_paths::db_path;
use isomfolio_core::indexing::thumbnail::{
    create_worker_pool, thumbnail_cache_path, ThumbnailPool,
};
use isomfolio_core::indexing::types::FileEvent;
use isomfolio_core::indexing::watcher::{create_watcher, FileWatcher};
use isomfolio_core::models::SearchQuery;
use isomfolio_core::models::{Album, AlbumId, AlbumKind, AssetFile, SortField, ThumbnailState};
use isomfolio_core::search::query_engine::{execute_manual_album_search, execute_search};
use isomfolio_core::storage::db;
use isomfolio_core::Connection;

pub struct App {
    pub conn: Option<Arc<Mutex<Connection>>>,
    pub catalog_dir: String,

    pub view_mode: ViewMode,
    pub loupe_idx: usize,

    pub folders: Vec<(String, String, usize)>,
    pub albums: Vec<Album>,
    pub album_counts: HashMap<String, usize>,
    pub selected_item: SidebarItem,
    pub drag_hover_album: Option<AlbumId>,

    pub files: Vec<AssetFile>,
    pub file_ratings: HashMap<String, i32>,
    pub thumbnails: HashMap<String, ThumbnailState>,
    pub grid_selected: HashSet<String>,
    pub tile_px: f32,
    pub anchor_idx: Option<usize>,

    pub scroll_y: f32,
    pub viewport_height: f32,
    pub viewport_width: f32,

    pub cursor: Point,
    pub drag: Option<DragState>,
    pub dragging_ids: HashSet<String>,
    pub modifiers: keyboard::Modifiers,

    pub thumbnail_pool: Option<ThumbnailPool>,
    pub thumbnail_tx: mpsc::SyncSender<ThumbnailEvent>,
    pub thumbnail_rx: mpsc::Receiver<ThumbnailEvent>,

    pub watcher_tx: mpsc::SyncSender<FileEvent>,
    pub watcher_rx: mpsc::Receiver<FileEvent>,
    pub watchers: Vec<(String, FileWatcher)>,

    pub search_text: String,
    pub pending_search: Option<(String, Instant)>,
    pub create_album_input: Option<String>,
    pub rename_album_id: Option<AlbumId>,
    pub rename_album_input: String,

    pub sort_by: SortField,
    pub sort_asc: bool,

    pub criteria: CriteriaState,
    pub detail: DetailState,

    pub thumbnail_pending: usize,
    pub thumbnail_total: usize,
    pub thumbnail_start_at: Option<Instant>,
    pub thumbnail_done_at: Option<Instant>,

    pub status: String,
    pub is_scanning: bool,
    pub scan_pending: bool,

    pub show_welcome: bool,
    pub recent_catalogs: Vec<String>,
    pub selected_recent_catalog: Option<String>,
    pub show_new_catalog_modal: bool,
    pub new_catalog_dir: Option<String>,
    pub new_catalog_name: String,
    pub album_pending_delete: Option<AlbumId>,
    pub folder_pending_remove: Option<String>,
    pub sidebar_scroll_y: f32,

    pub last_click_time: Option<Instant>,
    pub pending_album_select: Option<AlbumId>,
    pub last_scanned_path: Option<String>,
    pub remove_from_album_pending: bool,
    pub smart_album_dirty: bool,
    pub context_menu: Option<ContextMenuState>,
    pub hovered_sidebar_entity: Option<SidebarItem>,
    pub loupe_full_res: Option<(usize, iced::widget::image::Handle)>,
    pub loupe_prefetch: HashMap<usize, iced::widget::image::Handle>,
    pub thumbnail_handles: HashMap<String, iced::widget::image::Handle>,
    pub tag_browser: Option<TagBrowserState>,

    pub sidebar_width: f32,
    pub sidebar_resizing: bool,

    pub addons: Vec<Arc<AddonProcess>>,
    pub settings: SettingsState,

    pub face_clusters: Vec<isomfolio_core::models::FaceClusterSummary>,
    pub rename_face_cluster_id: Option<String>,
    pub rename_face_cluster_input: String,
}

impl App {
    pub fn new(catalog_dir: Option<String>) -> (Self, Task<Msg>) {
        let (tx, rx) = mpsc::sync_channel::<ThumbnailEvent>(500);
        let (wtx, wrx) = mpsc::sync_channel::<FileEvent>(200);

        let recent_catalogs = isomfolio_core::app_paths::read_recent_catalogs();

        let (catalog_dir_str, conn, initial_status, show_welcome, task) = match catalog_dir {
            Some(dir) => {
                isomfolio_core::app_paths::ensure_directories(&dir);
                let conn = db::open_database(&db_path(&dir))
                    .ok()
                    .map(|c| Arc::new(Mutex::new(c)));
                let status = if conn.is_none() {
                    "Error: could not open database — check permissions".to_string()
                } else {
                    String::new()
                };
                (dir, conn, status, false, Task::done(Msg::CatalogReady))
            }
            None => (String::new(), None, String::new(), true, Task::none()),
        };

        let app = App {
            conn,
            catalog_dir: catalog_dir_str,
            view_mode: ViewMode::Browse,
            loupe_idx: 0,
            folders: Vec::new(),
            albums: Vec::new(),
            album_counts: HashMap::new(),
            selected_item: SidebarItem::AllFiles,
            drag_hover_album: None,
            files: Vec::new(),
            file_ratings: HashMap::new(),
            thumbnails: HashMap::new(),
            grid_selected: HashSet::new(),
            tile_px: 180.0,
            anchor_idx: None,
            scroll_y: 0.0,
            viewport_height: 600.0,
            viewport_width: 1060.0,
            cursor: Point::ORIGIN,
            drag: None,
            dragging_ids: HashSet::new(),
            modifiers: keyboard::Modifiers::default(),
            thumbnail_pool: None,
            thumbnail_tx: tx,
            thumbnail_rx: rx,
            watcher_tx: wtx,
            watcher_rx: wrx,
            watchers: Vec::new(),
            search_text: String::new(),
            pending_search: None,
            create_album_input: None,
            rename_album_id: None,
            rename_album_input: String::new(),
            sort_by: SortField::Name,
            sort_asc: true,
            criteria: CriteriaState::default(),
            detail: DetailState::default(),
            thumbnail_pending: 0,
            thumbnail_total: 0,
            thumbnail_start_at: None,
            thumbnail_done_at: None,
            status: initial_status,
            is_scanning: false,
            scan_pending: false,
            show_welcome,
            recent_catalogs,
            selected_recent_catalog: None,
            show_new_catalog_modal: false,
            new_catalog_dir: None,
            new_catalog_name: String::new(),
            album_pending_delete: None,
            folder_pending_remove: None,
            sidebar_scroll_y: 0.0,
            last_click_time: None,
            pending_album_select: None,
            last_scanned_path: None,
            remove_from_album_pending: false,
            smart_album_dirty: false,
            context_menu: None,
            hovered_sidebar_entity: None,
            loupe_full_res: None,
            loupe_prefetch: HashMap::new(),
            thumbnail_handles: HashMap::new(),
            tag_browser: None,
            sidebar_width: SIDEBAR_WIDTH,
            sidebar_resizing: false,
            addons: Vec::new(),
            settings: SettingsState::default(),
            face_clusters: Vec::new(),
            rename_face_cluster_id: None,
            rename_face_cluster_input: String::new(),
        };

        (app, task)
    }

    pub(crate) fn resize_to_main() -> Task<Msg> {
        iced::window::oldest().then(|opt_id| match opt_id {
            Some(id) => iced::window::resize(id, Size::new(1280.0, 800.0)),
            None => Task::none(),
        })
    }

    pub fn window_title(&self) -> String {
        if self.catalog_dir.is_empty() {
            return "IsomFolio".to_string();
        }
        let name = std::path::Path::new(&self.catalog_dir)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("IsomFolio");
        format!("IsomFolio — {name}")
    }

    pub fn cols(&self) -> usize {
        let detail_w = if self.detail.show { SIDEBAR_WIDTH } else { 0.0 };
        let avail = (self.viewport_width - 2.0 * GRID_PADDING - detail_w).max(0.0);
        ((avail + TILE_GAP) / (self.tile_px + TILE_GAP)) as usize
    }

    pub fn criteria_panel_height(&self) -> f32 {
        if !self.criteria.show {
            return 0.0;
        }
        let rows = CRITERIA_ROW_COUNT as f32;
        let spacing = (CRITERIA_ROW_COUNT - 1) as f32 * 6.0;
        let action_row = if self.criteria_has_any() {
            CRITERIA_ROW_HEIGHT + 6.0
        } else {
            0.0
        };
        rows * CRITERIA_ROW_HEIGHT + spacing + CRITERIA_PADDING + action_row
    }

    pub fn criteria_has_any(&self) -> bool {
        use isomfolio_core::models::FlagFilter;
        !self.criteria.tags.is_empty()
            || !self.criteria.exts.is_empty()
            || !self.criteria.date_from.is_empty()
            || !self.criteria.date_to.is_empty()
            || self.criteria.flag_filter != FlagFilter::All
            || self.criteria.rating_min.is_some()
            || self.criteria.hide_rejects
    }

    pub fn current_album_is_smart(&self) -> bool {
        if let SidebarItem::Album(ref id) = self.selected_item {
            self.albums
                .iter()
                .find(|a| &a.id == id)
                .map(|a| matches!(a.kind, AlbumKind::Smart(_)))
                .unwrap_or(false)
        } else {
            false
        }
    }

    pub fn detail_file(&self) -> Option<&AssetFile> {
        let id = self.detail.file_id.as_deref()?;
        self.files.iter().find(|f| f.id == id)
    }

    pub fn build_search_query(&self) -> SearchQuery {
        let text_opt = {
            let t = self.search_text.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        };
        use isomfolio_core::models::FlagFilter;
        let effective_flag = if self.criteria.hide_rejects && self.criteria.flag_filter == FlagFilter::All {
            FlagFilter::NotReject
        } else {
            self.criteria.flag_filter
        };
        SearchQuery {
            text: text_opt,
            tags: self.criteria.tags.clone(),
            extensions: self.criteria.exts.iter().cloned().collect(),
            date_from: parse_date_str(&self.criteria.date_from),
            date_to: parse_date_str(&self.criteria.date_to),
            sort_by: self.sort_by,
            sort_asc: self.sort_asc,
            flag_filter: effective_flag,
            rating_min: self.criteria.rating_min,
            ..Default::default()
        }
    }

    pub(crate) fn start_thumbnail_pool(&mut self) {
        if self.thumbnail_pool.is_some() {
            return;
        }
        let tx_ready = self.thumbnail_tx.clone();
        let tx_failed = self.thumbnail_tx.clone();
        let catalog_dir = self.catalog_dir.clone();
        let concurrency = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        self.thumbnail_pool = Some(create_worker_pool(
            &catalog_dir,
            concurrency,
            move |fid, path| {
                let _ = tx_ready.try_send(ThumbnailEvent::Ready(fid, path));
            },
            move |fid, err| {
                let _ = tx_failed.try_send(ThumbnailEvent::Failed(fid, err));
            },
        ));
    }

    pub(crate) fn enqueue_thumbnails(&mut self) {
        let Some(pool) = &self.thumbnail_pool else {
            return;
        };
        let catalog_dir = self.catalog_dir.clone();
        let mut newly_enqueued = 0usize;
        for (priority, file) in self.files.iter().enumerate() {
            if !self.thumbnails.contains_key(&file.id) {
                self.thumbnails.insert(file.id.clone(), ThumbnailState::Pending);
                let cache = thumbnail_cache_path(&catalog_dir, &file.id);
                if std::path::Path::new(&cache).exists() {
                    self.thumbnails.insert(file.id.clone(), ThumbnailState::Ready(cache));
                } else {
                    pool.enqueue(&file.id, &file.path, priority as i32);
                    newly_enqueued += 1;
                }
            }
        }
        if newly_enqueued > 0 {
            if self.thumbnail_pending == 0 {
                self.thumbnail_total = newly_enqueued;
                self.thumbnail_start_at = Some(Instant::now());
                self.thumbnail_done_at = None;
            } else {
                self.thumbnail_total += newly_enqueued;
            }
            self.thumbnail_pending += newly_enqueued;
        }
    }

    pub(crate) fn start_watchers_for_folders(&mut self) {
        let current: HashSet<String> = self.watchers.iter().map(|(p, _)| p.clone()).collect();
        let new_paths: Vec<String> = self
            .folders
            .iter()
            .filter(|(p, _, _)| !current.contains(p.as_str()))
            .map(|(p, _, _)| p.clone())
            .collect();
        for path in new_paths {
            let tx = self.watcher_tx.clone();
            if let Ok(w) = create_watcher(&path, move |ev| {
                let _ = tx.try_send(ev);
            }) {
                self.watchers.push((path, w));
            }
        }
        let folder_set: HashSet<&str> = self.folders.iter().map(|(p, _, _)| p.as_str()).collect();
        self.watchers
            .retain(|(p, _)| folder_set.contains(p.as_str()));
    }

    pub fn load_files_task(&self) -> Task<Msg> {
        let Some(conn) = self.conn.clone() else {
            return Task::done(Msg::FilesLoaded(Vec::new()));
        };
        let item = self.selected_item.clone();
        let query = self.build_search_query();
        let is_smart = self.current_album_is_smart();

        Task::perform(
            async move {
                let guard = conn.lock().unwrap();
                match item {
                    SidebarItem::AllFiles => execute_search(&guard, &query).unwrap_or_default(),
                    SidebarItem::Folder(path) => {
                        let q = SearchQuery {
                            folder_path: Some(path),
                            folder_recursive: true,
                            ..query
                        };
                        execute_search(&guard, &q).unwrap_or_default()
                    }
                    SidebarItem::Album(album_id) => {
                        if is_smart {
                            execute_search(&guard, &query).unwrap_or_default()
                        } else {
                            execute_manual_album_search(&guard, &album_id, &query)
                                .unwrap_or_default()
                        }
                    }
                    SidebarItem::FaceCluster(cluster_id) => {
                        db::get_files_in_face_cluster(&guard, &cluster_id).unwrap_or_default()
                    }
                }
            },
            Msg::FilesLoaded,
        )
    }

    pub(crate) fn load_sidebar_task(&self) -> Task<Msg> {
        let Some(conn) = self.conn.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock().unwrap();
                let raw_folders = db::get_folder_counts(&g).unwrap_or_default();
                let albums = db::get_all_albums(&g).unwrap_or_default();
                let album_counts = db::get_all_album_file_counts(&g).unwrap_or_default();
                drop(g);
                let folders = raw_folders
                    .into_iter()
                    .map(|(path, count)| {
                        let display = std::fs::canonicalize(&path)
                            .ok()
                            .and_then(|p| {
                                p.file_name().map(|n| n.to_string_lossy().into_owned())
                            })
                            .unwrap_or_else(|| {
                                std::path::Path::new(&path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or(&path)
                                    .to_string()
                            });
                        (path, display, count)
                    })
                    .collect();
                (folders, albums, album_counts)
            },
            |(folders, albums, album_counts)| Msg::SidebarLoaded {
                folders,
                albums,
                album_counts,
            },
        )
    }

    pub(crate) fn maybe_load_detail(&self) -> Task<Msg> {
        if !self.detail.show {
            return Task::none();
        }
        if self.grid_selected.len() != 1 {
            return Task::none();
        }
        let file_id = self.grid_selected.iter().next().unwrap().clone();
        if self.detail.file_id.as_deref() == Some(file_id.as_str()) {
            return Task::none();
        }
        let Some(conn) = self.conn.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock().unwrap();
                let tags = db::get_tags_for_file(&g, &file_id).unwrap_or_default();
                let meta_opt = db::get_metadata(&g, &file_id).ok().flatten();
                let (rating, label, title, exif_tech) = match meta_opt {
                    Some(m) => (
                        m.xmp.as_ref().and_then(|x| x.core.rating),
                        m.xmp.as_ref().and_then(|x| x.core.label.clone()),
                        m.xmp.as_ref().and_then(|x| x.dublin_core.title.clone()),
                        m.exif_tech,
                    ),
                    None => (None, None, None, None),
                };
                (file_id, tags, rating, label, title, exif_tech)
            },
            |(file_id, tags, rating, label, title, exif_tech)| Msg::DetailLoaded {
                file_id,
                tags,
                rating,
                label,
                title,
                exif_tech,
            },
        )
    }

    pub(crate) fn load_all_tags_task(&self) -> Task<Msg> {
        let Some(conn) = self.conn.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock().unwrap();
                db::get_all_tags(&g)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(t, _)| t)
                    .collect()
            },
            Msg::AllTagsLoaded,
        )
    }

    pub(crate) fn load_ratings_task(&self) -> Task<Msg> {
        let Some(conn) = self.conn.clone() else {
            return Task::none();
        };
        let file_ids: Vec<String> = self.files.iter().map(|f| f.id.clone()).collect();
        if file_ids.is_empty() {
            return Task::done(Msg::RatingsLoaded(HashMap::new()));
        }
        Task::perform(
            async move {
                let g = conn.lock().unwrap();
                file_ids
                    .iter()
                    .filter_map(|id| {
                        let meta = db::get_metadata(&g, id).ok()??;
                        let r = meta.xmp.as_ref()?.core.rating?;
                        (r > 0).then(|| (id.clone(), r))
                    })
                    .collect()
            },
            Msg::RatingsLoaded,
        )
    }

    pub(crate) fn load_tag_browser_task(&self) -> Task<Msg> {
        let Some(conn) = self.conn.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock().unwrap();
                db::get_all_tags(&g).unwrap_or_default()
            },
            Msg::TagBrowserLoaded,
        )
    }

    pub fn tile_index_at(&self, pos: Point) -> Option<usize> {
        let rel_x = pos.x - self.sidebar_width - SIDEBAR_HANDLE_WIDTH - GRID_PADDING;
        let criteria_h = self.criteria_panel_height();
        let rel_y = pos.y + self.scroll_y - SEARCH_BAR_HEIGHT - criteria_h - GRID_PADDING;
        if rel_x < 0.0 || rel_y < 0.0 {
            return None;
        }
        let step = self.tile_px + TILE_GAP;
        let col = (rel_x / step) as usize;
        let cols = self.cols();
        if col >= cols {
            return None;
        }
        let row = (rel_y / step) as usize;
        let in_x = rel_x - col as f32 * step;
        let in_y = rel_y - row as f32 * step;
        if in_x > self.tile_px || in_y > self.tile_px {
            return None;
        }
        let idx = row * cols + col;
        if idx < self.files.len() {
            Some(idx)
        } else {
            None
        }
    }

    pub fn subscription(&self) -> Subscription<Msg> {
        let tick_sub = iced::time::every(std::time::Duration::from_millis(41)).map(|_| Msg::Tick);

        let event_sub = event::listen_with(|event, status, _id| {
            let ignored = status == iced::event::Status::Ignored;
            match event {
                Event::Mouse(mouse::Event::CursorMoved { position }) => Some(Msg::MouseMoved(position)),
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    Some(Msg::MousePressed)
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    Some(Msg::MouseReleased)
                }
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                    Some(Msg::MouseRightClicked)
                }
                Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => Some(Msg::ModifiersChanged(m)),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                }) => Some(Msg::EscapePressed),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Enter),
                    ..
                }) if ignored => Some(Msg::OpenLoupe),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Space),
                    ..
                }) if ignored => Some(Msg::OpenLoupe),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    modifiers,
                    ..
                }) if ignored && modifiers.command() && c.as_str() == "=" => Some(Msg::TileSizeUp),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    modifiers,
                    ..
                }) if ignored && modifiers.command() && c.as_str() == "-" => Some(Msg::TileSizeDown),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    ..
                }) if ignored && c.as_str() == "i" => Some(Msg::ToggleDetail),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    ..
                }) if ignored && c.as_str() == "p" => Some(Msg::SetFlag(isomfolio_core::models::Flag::Pick)),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    ..
                }) if ignored && c.as_str() == "x" => Some(Msg::SetFlag(isomfolio_core::models::Flag::Reject)),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    ..
                }) if ignored && c.as_str() == "u" => Some(Msg::SetFlag(isomfolio_core::models::Flag::Unflagged)),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    ..
                }) if ignored && c.as_str() == "0" => Some(Msg::SetRating(None)),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    ..
                }) if ignored && c.as_str() == "1" => Some(Msg::SetRating(Some(1))),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    ..
                }) if ignored && c.as_str() == "2" => Some(Msg::SetRating(Some(2))),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    ..
                }) if ignored && c.as_str() == "3" => Some(Msg::SetRating(Some(3))),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    ..
                }) if ignored && c.as_str() == "4" => Some(Msg::SetRating(Some(4))),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    ..
                }) if ignored && c.as_str() == "5" => Some(Msg::SetRating(Some(5))),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref c),
                    ..
                }) if ignored && c.as_str() == "\\" => Some(Msg::ToggleHideRejects),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::ArrowLeft),
                    ..
                }) if ignored => Some(Msg::Navigate { dx: -1, dy: 0 }),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::ArrowRight),
                    ..
                }) if ignored => Some(Msg::Navigate { dx: 1, dy: 0 }),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::ArrowUp),
                    ..
                }) if ignored => Some(Msg::Navigate { dx: 0, dy: -1 }),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::ArrowDown),
                    ..
                }) if ignored => Some(Msg::Navigate { dx: 0, dy: 1 }),
                _ => None,
            }
        });

        Subscription::batch([tick_sub, event_sub])
    }
}

pub fn sort_field_label(f: SortField) -> &'static str {
    match f {
        SortField::Name => "Name",
        SortField::Date => "Date",
        SortField::Size => "Size",
        SortField::Ext => "Type",
    }
}

pub fn unix_to_date_str(ts: i64) -> String {
    let days = ts / 86400;
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn parse_date_str(s: &str) -> Option<i64> {
    if s.trim().is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: i64 = parts[0].parse().ok()?;
    let m: i64 = parts[1].parse().ok()?;
    let d: i64 = parts[2].parse().ok()?;
    if m < 1 || m > 12 || d < 1 || d > 31 {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days * 86400)
}

pub fn format_file_size(bytes: i64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kb = bytes / 1024;
    if kb < 1024 {
        return format!("{kb} KB");
    }
    let mb = kb / 1024;
    if mb < 1024 {
        return format!("{mb} MB");
    }
    format!("{} GB", mb / 1024)
}
