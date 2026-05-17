use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use iced::{
    Event, Point, Subscription, Task,
    event, keyboard, mouse,
};

use isomfolio_core::file_index::compute_file_id;
use isomfolio_core::indexing::types::FileEvent;
use isomfolio_core::indexing::watcher::{FileWatcher, create_watcher};
use isomfolio_core::models::{Album, AlbumId, AlbumKind, AssetFile, SortField, ThumbnailState};
use isomfolio_core::path_utils::normalize_path;
use isomfolio_core::storage::db;
use isomfolio_core::app_paths::db_path;
use isomfolio_core::indexing::thumbnail::{ThumbnailPool, create_worker_pool, thumbnail_cache_path};
use isomfolio_core::indexing::scanner;
use isomfolio_core::Connection;
use isomfolio_core::search::query_engine::{execute_search, execute_manual_album_search};
use isomfolio_core::models::SearchQuery;

pub const SIDEBAR_WIDTH: f32 = 220.0;
pub const GRID_PADDING: f32 = 12.0;
pub const TILE_GAP: f32 = 8.0;
pub const ALBUM_ITEM_HEIGHT: f32 = 44.0;
pub const DRAG_THRESHOLD: f32 = 6.0;
pub const BUFFER_ROWS: usize = 2;
pub const SEARCH_BAR_HEIGHT: f32 = 40.0;
pub const CRITERIA_ROW_HEIGHT: f32 = 32.0;
pub const CRITERIA_ROW_COUNT: usize = 3;
pub const CRITERIA_PADDING: f32 = 18.0;

#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    Browse,
    Loupe,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SidebarItem {
    AllFiles,
    Folder(String),
    Album(AlbumId),
}

#[derive(Debug, Clone)]
pub struct DragState {
    pub origin_idx: usize,
    pub start: Point,
    pub cursor: Point,
    pub active: bool,
}

#[derive(Debug)]
pub enum ThumbnailEvent {
    Ready(String, String),
    Failed(String, String),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Msg {
    CatalogReady,

    SidebarItemClicked(SidebarItem),

    FilesLoaded(Vec<AssetFile>),
    SidebarLoaded {
        folders: Vec<(String, usize)>,
        albums: Vec<Album>,
        album_counts: HashMap<String, usize>,
    },

    TileSizeUp,
    TileSizeDown,

    MouseMoved(Point),
    MousePressed,
    MouseReleased,
    ModifiersChanged(keyboard::Modifiers),
    EscapePressed,
    Navigate { dx: i32, dy: i32 },
    OpenLoupe,

    Scrolled { y: f32, height: f32, width: f32 },

    DroppedToAlbum(AlbumId),
    DropCompleted,

    ScanPickFolder,
    ScanStart(String),
    ScanComplete(usize),

    StartCreateAlbum,
    CreateAlbumInputChanged(String),
    ConfirmCreateAlbum,
    CancelCreateAlbum,
    AlbumCreated,
    AlbumRenamed,
    SmartAlbumUpdated,
    FilesRemovedFromAlbum,

    StartRenameAlbum(AlbumId),
    RenameAlbumInputChanged(String),
    ConfirmRenameAlbum,

    DeleteAlbum(AlbumId),
    AlbumDeleted,

    RemoveFolder(String),
    FolderRemoved,

    RemoveFromAlbum,

    SortFieldCycle,
    SortDirToggle,

    SearchChanged(String),

    // Criteria panel
    ToggleCriteria,
    CriteriaTagInputChanged(String),
    AddCriteriaTag,
    RemoveCriteriaTag(String),
    CriteriaDateFromChanged(String),
    CriteriaDateToChanged(String),
    ToggleCriteriaExt(String),
    ClearCriteria,

    // Smart albums
    SaveAsSmartAlbum,
    SmartAlbumNameChanged(String),
    ConfirmSmartAlbum,
    UpdateSmartAlbum,

    // Detail panel
    ToggleDetail,
    DetailLoaded {
        file_id: String,
        tags: Vec<String>,
        rating: Option<i32>,
        label: Option<String>,
        title: Option<String>,
    },
    DetailTagInputChanged(String),
    AddDetailTag,
    RemoveDetailTag(String),
    SetDetailRating(i32),

    Reload,
    DbError(String),
    Tick,
    DragHoverAlbum(Option<AlbumId>),

    // Catalog management
    PickOpenCatalog,
    OpenCatalogPicked(String),
    PickNewCatalogDir,
    NewCatalogDirPicked(String),
    NewCatalogNameChanged(String),
    ConfirmNewCatalog,
    OpenCatalog(String),

    // Confirm destructive actions
    RequestDeleteAlbum(AlbumId),
    CancelDeleteAlbum,
    RequestRemoveFolder(String),
    CancelRemoveFolder,

    ScanDialogDone(Option<String>),
    SortCycleAll,
    NoOp,
}

pub struct CriteriaState {
    pub show: bool,
    pub tags: Vec<String>,
    pub tag_input: String,
    pub date_from: String,
    pub date_to: String,
    pub exts: HashSet<String>,
    pub save_smart_input: Option<String>,
}

impl Default for CriteriaState {
    fn default() -> Self {
        Self {
            show: false,
            tags: Vec::new(),
            tag_input: String::new(),
            date_from: String::new(),
            date_to: String::new(),
            exts: HashSet::new(),
            save_smart_input: None,
        }
    }
}

pub struct DetailState {
    pub show: bool,
    pub file_id: Option<String>,
    pub tags: Vec<String>,
    pub tag_input: String,
    pub rating: Option<i32>,
    pub label: Option<String>,
    pub title: Option<String>,
}

impl Default for DetailState {
    fn default() -> Self {
        Self {
            show: false,
            file_id: None,
            tags: Vec::new(),
            tag_input: String::new(),
            rating: None,
            label: None,
            title: None,
        }
    }
}

pub struct App {
    pub conn: Option<Arc<Mutex<Connection>>>,
    pub catalog_dir: String,

    pub view_mode: ViewMode,
    pub loupe_idx: usize,

    pub folders: Vec<(String, usize)>,
    pub albums: Vec<Album>,
    pub album_counts: HashMap<String, usize>,
    pub selected_item: SidebarItem,
    pub drag_hover_album: Option<AlbumId>,

    pub files: Vec<AssetFile>,
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

    pub status: String,
    pub is_scanning: bool,
    pub scan_pending: bool,

    pub show_welcome: bool,
    pub recent_catalogs: Vec<String>,
    pub new_catalog_dir: Option<String>,
    pub new_catalog_name: String,
    pub album_pending_delete: Option<AlbumId>,
    pub folder_pending_remove: Option<String>,
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
            status: initial_status,
            is_scanning: false,
            scan_pending: false,
            show_welcome,
            recent_catalogs,
            new_catalog_dir: None,
            new_catalog_name: String::new(),
            album_pending_delete: None,
            folder_pending_remove: None,
        };

        (app, task)
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
        let action_row = if self.criteria_has_any() { CRITERIA_ROW_HEIGHT + 6.0 } else { 0.0 };
        rows * CRITERIA_ROW_HEIGHT + spacing + CRITERIA_PADDING + action_row
    }

    pub fn criteria_has_any(&self) -> bool {
        !self.criteria.tags.is_empty()
            || !self.criteria.exts.is_empty()
            || !self.criteria.date_from.is_empty()
            || !self.criteria.date_to.is_empty()
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
            if t.is_empty() { None } else { Some(t.to_string()) }
        };
        SearchQuery {
            text: text_opt,
            tags: self.criteria.tags.clone(),
            extensions: self.criteria.exts.iter().cloned().collect(),
            date_from: parse_date_str(&self.criteria.date_from),
            date_to: parse_date_str(&self.criteria.date_to),
            sort_by: self.sort_by,
            sort_asc: self.sort_asc,
            ..Default::default()
        }
    }

    fn start_thumbnail_pool(&mut self) {
        if self.thumbnail_pool.is_some() {
            return;
        }
        let tx_ready = self.thumbnail_tx.clone();
        let tx_failed = self.thumbnail_tx.clone();
        let catalog_dir = self.catalog_dir.clone();
        self.thumbnail_pool = Some(create_worker_pool(
            &catalog_dir,
            4,
            move |fid, path| {
                let _ = tx_ready.try_send(ThumbnailEvent::Ready(fid, path));
            },
            move |fid, err| {
                let _ = tx_failed.try_send(ThumbnailEvent::Failed(fid, err));
            },
        ));
    }

    fn enqueue_thumbnails(&mut self) {
        let Some(pool) = &self.thumbnail_pool else { return };
        let catalog_dir = self.catalog_dir.clone();
        for (priority, file) in self.files.iter().enumerate() {
            if !self.thumbnails.contains_key(&file.id) {
                self.thumbnails.insert(file.id.clone(), ThumbnailState::Pending);
                let cache = thumbnail_cache_path(&catalog_dir, &file.id);
                if std::path::Path::new(&cache).exists() {
                    self.thumbnails.insert(file.id.clone(), ThumbnailState::Ready(cache));
                } else {
                    pool.enqueue(&file.id, &file.path, priority as i32);
                }
            }
        }
    }

    fn start_watchers_for_folders(&mut self) {
        let current: HashSet<String> = self.watchers.iter().map(|(p, _)| p.clone()).collect();
        let new_paths: Vec<String> = self.folders.iter()
            .filter(|(p, _)| !current.contains(p.as_str()))
            .map(|(p, _)| p.clone())
            .collect();
        for path in new_paths {
            let tx = self.watcher_tx.clone();
            if let Ok(w) = create_watcher(&path, move |ev| {
                let _ = tx.try_send(ev);
            }) {
                self.watchers.push((path, w));
            }
        }
        let folder_set: HashSet<&str> = self.folders.iter().map(|(p, _)| p.as_str()).collect();
        self.watchers.retain(|(p, _)| folder_set.contains(p.as_str()));
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
                    SidebarItem::AllFiles => {
                        execute_search(&guard, &query).unwrap_or_default()
                    }
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
                            execute_manual_album_search(&guard, &album_id).unwrap_or_default()
                        }
                    }
                }
            },
            Msg::FilesLoaded,
        )
    }

    fn load_sidebar_task(&self) -> Task<Msg> {
        let Some(conn) = self.conn.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let g = conn.lock().unwrap();
                let folders = db::get_folder_counts(&g).unwrap_or_default();
                let albums = db::get_all_albums(&g).unwrap_or_default();
                let album_counts = db::get_all_album_file_counts(&g).unwrap_or_default();
                (folders, albums, album_counts)
            },
            |(folders, albums, album_counts)| Msg::SidebarLoaded { folders, albums, album_counts },
        )
    }

    fn maybe_load_detail(&self) -> Task<Msg> {
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
        let Some(conn) = self.conn.clone() else { return Task::none(); };
        Task::perform(
            async move {
                let g = conn.lock().unwrap();
                let tags = db::get_tags_for_file(&g, &file_id).unwrap_or_default();
                let meta_opt = db::get_metadata(&g, &file_id).ok().flatten();
                let (rating, label, title) = match meta_opt {
                    Some(m) => (
                        m.xmp.as_ref().and_then(|x| x.core.rating),
                        m.xmp.as_ref().and_then(|x| x.core.label.clone()),
                        m.xmp.as_ref().and_then(|x| x.dublin_core.title.clone()),
                    ),
                    None => (None, None, None),
                };
                (file_id, tags, rating, label, title)
            },
            |(file_id, tags, rating, label, title)| Msg::DetailLoaded {
                file_id,
                tags,
                rating,
                label,
                title,
            },
        )
    }

    pub fn tile_index_at(&self, pos: Point) -> Option<usize> {
        let rel_x = pos.x - SIDEBAR_WIDTH - GRID_PADDING;
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
        if idx < self.files.len() { Some(idx) } else { None }
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::CatalogReady => {
                self.start_thumbnail_pool();
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

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
                            self.criteria.show = true;
                        }
                    }
                }
                self.selected_item = item;
                self.files.clear();
                self.scroll_y = 0.0;
                self.loupe_idx = 0;
                self.grid_selected.clear();
                self.drag = None;
                self.dragging_ids.clear();
                self.criteria.save_smart_input = None;
                self.detail.file_id = None;
                self.load_files_task()
            }

            Msg::FilesLoaded(files) => {
                self.files = files;
                self.enqueue_thumbnails();
                self.status = format!("{} photo(s)", self.files.len());
                self.maybe_load_detail()
            }

            Msg::SidebarLoaded { folders, albums, album_counts } => {
                self.folders = folders;
                self.albums = albums;
                self.album_counts = album_counts;
                self.start_watchers_for_folders();
                Task::none()
            }

            Msg::TileSizeUp => {
                self.tile_px = (self.tile_px + 40.0).min(400.0);
                Task::none()
            }

            Msg::TileSizeDown => {
                self.tile_px = (self.tile_px - 40.0).max(80.0);
                Task::none()
            }

            Msg::Navigate { dx, dy } => {
                if matches!(self.view_mode, ViewMode::Loupe) {
                    let total = self.files.len();
                    if total == 0 { return Task::none(); }
                    let delta = dx + dy;
                    self.loupe_idx = (self.loupe_idx as i32 + delta)
                        .rem_euclid(total as i32) as usize;
                    return Task::none();
                }
                let cols = self.cols().max(1) as i32;
                let total = self.files.len() as i32;
                if total == 0 { return Task::none(); }
                let current = self.anchor_idx.unwrap_or(0) as i32;
                let row = current / cols;
                let col = current % cols;
                let new_col = (col + dx).clamp(0, cols - 1);
                let new_row = (row + dy).clamp(0, (total - 1) / cols);
                let new_idx = (new_row * cols + new_col).min(total - 1) as usize;
                self.anchor_idx = Some(new_idx);
                self.grid_selected.clear();
                if let Some(f) = self.files.get(new_idx) {
                    self.grid_selected.insert(f.id.clone());
                }
                self.maybe_load_detail()
            }

            Msg::OpenLoupe => {
                match self.view_mode {
                    ViewMode::Loupe => {
                        self.view_mode = ViewMode::Browse;
                    }
                    ViewMode::Browse => {
                        if !self.files.is_empty() {
                            self.loupe_idx =
                                self.anchor_idx.unwrap_or(0).min(self.files.len() - 1);
                            self.view_mode = ViewMode::Loupe;
                        }
                    }
                }
                Task::none()
            }

            Msg::MouseMoved(pos) => {
                self.cursor = pos;
                if let Some(ref mut d) = self.drag {
                    d.cursor = pos;
                    if !d.active {
                        let dx = pos.x - d.start.x;
                        let dy = pos.y - d.start.y;
                        if (dx * dx + dy * dy).sqrt() > DRAG_THRESHOLD {
                            d.active = true;
                            let origin_idx = d.origin_idx;
                            let origin_id = self.files[origin_idx].id.clone();
                            self.dragging_ids = if self.grid_selected.contains(&origin_id) {
                                self.grid_selected.clone()
                            } else {
                                [origin_id].into()
                            };
                        }
                    }
                }
                Task::none()
            }

            Msg::MousePressed => {
                let pos = self.cursor;
                if matches!(self.view_mode, ViewMode::Browse) {
                    if let Some(idx) = self.tile_index_at(pos) {
                        let file_id = self.files[idx].id.clone();
                        let mods = self.modifiers;
                        if mods.command() {
                            if self.grid_selected.contains(&file_id) {
                                self.grid_selected.remove(&file_id);
                            } else {
                                self.grid_selected.insert(file_id.clone());
                                self.anchor_idx = Some(idx);
                            }
                        } else if mods.shift() {
                            let anchor = self.anchor_idx.unwrap_or(idx);
                            let lo = anchor.min(idx);
                            let hi = anchor.max(idx);
                            for i in lo..=hi {
                                if let Some(f) = self.files.get(i) {
                                    self.grid_selected.insert(f.id.clone());
                                }
                            }
                        } else if !self.grid_selected.contains(&file_id) {
                            self.grid_selected.clear();
                            self.grid_selected.insert(file_id);
                            self.anchor_idx = Some(idx);
                        }
                        self.drag = Some(DragState {
                            origin_idx: idx,
                            start: pos,
                            cursor: pos,
                            active: false,
                        });
                    } else if pos.x > SIDEBAR_WIDTH {
                        let mods = self.modifiers;
                        if !mods.command() && !mods.shift() {
                            self.grid_selected.clear();
                            self.anchor_idx = None;
                        }
                    }
                }
                if self.detail.show && self.grid_selected.len() != 1 {
                    self.detail.file_id = None;
                    self.detail.tags.clear();
                    self.detail.rating = None;
                    self.detail.label = None;
                    self.detail.title = None;
                }
                Task::none()
            }

            Msg::MouseReleased => {
                let drop_task = if self.drag.as_ref().map_or(false, |d| d.active) {
                    self.drag_hover_album.clone().map(|id| Task::done(Msg::DroppedToAlbum(id)))
                } else {
                    None
                };
                self.drag = None;
                self.dragging_ids.clear();
                self.drag_hover_album = None;

                let detail_task = self.maybe_load_detail();
                match drop_task {
                    Some(t) => Task::batch([t, detail_task]),
                    None => detail_task,
                }
            }

            Msg::ModifiersChanged(m) => {
                self.modifiers = m;
                Task::none()
            }

            Msg::EscapePressed => {
                if matches!(self.view_mode, ViewMode::Loupe) {
                    self.view_mode = ViewMode::Browse;
                    return Task::none();
                }
                self.create_album_input = None;
                self.rename_album_id = None;
                self.criteria.save_smart_input = None;
                Task::none()
            }

            Msg::Scrolled { y, height, width } => {
                self.scroll_y = y;
                self.viewport_height = height;
                self.viewport_width = width;
                Task::none()
            }

            Msg::DroppedToAlbum(album_id) => {
                let ids: Vec<String> = self.dragging_ids.iter().cloned().collect();
                self.drag = None;
                self.dragging_ids.clear();
                self.drag_hover_album = None;
                let name = self.albums.iter()
                    .find(|a| a.id == album_id)
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                let count = ids.len();
                self.status = format!("Added {count} photo(s) to \"{name}\"");

                let Some(conn) = self.conn.clone() else { return Task::none(); };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        for fid in &ids {
                            let _ = db::add_file_to_album(&guard, &album_id, fid);
                        }
                    },
                    |()| Msg::DropCompleted,
                )
            }

            Msg::DropCompleted => self.load_sidebar_task(),

            Msg::ScanPickFolder => {
                if self.is_scanning || self.scan_pending {
                    return Task::none();
                }
                self.scan_pending = true;
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Choose folder to scan")
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_string_lossy().to_string())
                    },
                    Msg::ScanDialogDone,
                )
            }

            Msg::ScanDialogDone(opt) => {
                self.scan_pending = false;
                match opt {
                    Some(path) => Task::done(Msg::ScanStart(path)),
                    None => Task::none(),
                }
            }

            Msg::ScanStart(path) => {
                self.is_scanning = true;
                self.status = "Scanning…".to_string();
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                let wtx = self.watcher_tx.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let guard = conn.lock().unwrap();
                            scanner::scan_folder(&guard, &path, &|_| {}, &|prog| {
                                let _ = wtx.try_send(FileEvent::ScanProgress(prog));
                            })
                            .map(|r| r.total_count)
                            .unwrap_or(0)
                        })
                        .await
                        .unwrap_or(0)
                    },
                    Msg::ScanComplete,
                )
            }

            Msg::ScanComplete(count) => {
                self.is_scanning = false;
                self.status = format!("Scanned {count} photo(s)");
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::RequestRemoveFolder(path) => {
                self.folder_pending_remove = Some(path);
                Task::none()
            }

            Msg::CancelRemoveFolder => {
                self.folder_pending_remove = None;
                Task::none()
            }

            Msg::RemoveFolder(path) => {
                self.folder_pending_remove = None;
                self.folders.retain(|(p, _)| p != &path);
                self.watchers.retain(|(p, _)| p != &path);
                if self.selected_item == SidebarItem::Folder(path.clone()) {
                    self.selected_item = SidebarItem::AllFiles;
                    self.files.clear();
                }
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        let _ = db::delete_files_by_root_folder(&guard, &path);
                    },
                    |()| Msg::FolderRemoved,
                )
            }

            Msg::FolderRemoved => {
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::StartCreateAlbum => {
                self.create_album_input = Some(String::new());
                Task::none()
            }

            Msg::CreateAlbumInputChanged(s) => {
                self.create_album_input = Some(s);
                Task::none()
            }

            Msg::ConfirmCreateAlbum => {
                let name = self.create_album_input.take().unwrap_or_default();
                let name = name.trim().to_string();
                if name.is_empty() {
                    return Task::none();
                }
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                let album = Album {
                    id: new_album_id(),
                    name,
                    kind: AlbumKind::Manual,
                    sort_order: 0,
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        let _ = db::create_album(&guard, &album);
                    },
                    |()| Msg::AlbumCreated,
                )
            }

            Msg::CancelCreateAlbum => {
                self.create_album_input = None;
                Task::none()
            }

            Msg::AlbumCreated | Msg::AlbumRenamed | Msg::SmartAlbumUpdated => {
                self.load_sidebar_task()
            }

            Msg::FilesRemovedFromAlbum => {
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::StartRenameAlbum(album_id) => {
                let current_name = self.albums.iter()
                    .find(|a| a.id == album_id)
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                self.rename_album_id = Some(album_id);
                self.rename_album_input = current_name;
                Task::none()
            }

            Msg::RenameAlbumInputChanged(s) => {
                self.rename_album_input = s;
                Task::none()
            }

            Msg::ConfirmRenameAlbum => {
                let name = self.rename_album_input.trim().to_string();
                let Some(album_id) = self.rename_album_id.take() else {
                    return Task::none();
                };
                if name.is_empty() {
                    return Task::none();
                }
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        let _ = db::rename_album(&guard, &album_id, &name);
                    },
                    |()| Msg::AlbumRenamed,
                )
            }

            Msg::RequestDeleteAlbum(album_id) => {
                self.album_pending_delete = Some(album_id);
                Task::none()
            }

            Msg::CancelDeleteAlbum => {
                self.album_pending_delete = None;
                Task::none()
            }

            Msg::DeleteAlbum(album_id) => {
                self.album_pending_delete = None;
                if self.selected_item == SidebarItem::Album(album_id.clone()) {
                    self.selected_item = SidebarItem::AllFiles;
                    self.files.clear();
                }
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        let _ = db::delete_album(&guard, &album_id);
                    },
                    |()| Msg::AlbumDeleted,
                )
            }

            Msg::AlbumDeleted => {
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::RemoveFromAlbum => {
                let SidebarItem::Album(ref album_id) = self.selected_item else {
                    return Task::none();
                };
                let album_id = album_id.clone();
                let ids: Vec<String> = self.grid_selected.iter().cloned().collect();
                let count = ids.len();
                let name = self.albums.iter()
                    .find(|a| a.id == album_id)
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                self.status = format!("Removed {count} photo(s) from \"{name}\"");
                self.grid_selected.clear();
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        for fid in &ids {
                            let _ = db::remove_file_from_album(&guard, &album_id, fid);
                        }
                    },
                    |()| Msg::FilesRemovedFromAlbum,
                )
            }

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
                self.pending_search = Some((text, Instant::now()));
                Task::none()
            }

            // Criteria panel

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
                self.load_files_task()
            }

            Msg::RemoveCriteriaTag(tag) => {
                self.criteria.tags.retain(|t| t != &tag);
                self.load_files_task()
            }

            Msg::CriteriaDateFromChanged(s) => {
                self.criteria.date_from = s;
                self.load_files_task()
            }

            Msg::CriteriaDateToChanged(s) => {
                self.criteria.date_to = s;
                self.load_files_task()
            }

            Msg::ToggleCriteriaExt(ext) => {
                if self.criteria.exts.contains(&ext) {
                    self.criteria.exts.remove(&ext);
                } else {
                    self.criteria.exts.insert(ext);
                }
                self.load_files_task()
            }

            Msg::ClearCriteria => {
                self.criteria.tags.clear();
                self.criteria.date_from.clear();
                self.criteria.date_to.clear();
                self.criteria.exts.clear();
                self.load_files_task()
            }

            // Smart album handlers

            Msg::SaveAsSmartAlbum => {
                self.criteria.save_smart_input = Some(String::new());
                Task::none()
            }

            Msg::SmartAlbumNameChanged(s) => {
                self.criteria.save_smart_input = Some(s);
                Task::none()
            }

            Msg::ConfirmSmartAlbum => {
                let name = self.criteria.save_smart_input.take().unwrap_or_default();
                let name = name.trim().to_string();
                if name.is_empty() {
                    return Task::none();
                }
                let query = self.build_search_query();
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                let album = Album {
                    id: new_album_id(),
                    name,
                    kind: AlbumKind::Smart(query),
                    sort_order: 0,
                };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        let _ = db::create_album(&guard, &album);
                    },
                    |()| Msg::AlbumCreated,
                )
            }

            Msg::UpdateSmartAlbum => {
                let SidebarItem::Album(ref id) = self.selected_item else {
                    return Task::none();
                };
                let album_id = id.clone();
                let query = self.build_search_query();
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                Task::perform(
                    async move {
                        let guard = conn.lock().unwrap();
                        let _ = db::update_smart_album_query(&guard, &album_id, &query);
                    },
                    |()| Msg::SmartAlbumUpdated,
                )
            }

            // Detail panel handlers

            Msg::ToggleDetail => {
                self.detail.show = !self.detail.show;
                if self.detail.show {
                    self.detail.file_id = None;
                    self.maybe_load_detail()
                } else {
                    Task::none()
                }
            }

            Msg::DetailLoaded { file_id, tags, rating, label, title } => {
                self.detail.file_id = Some(file_id);
                self.detail.tags = tags;
                self.detail.rating = rating;
                self.detail.label = label;
                self.detail.title = title;
                Task::none()
            }

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
                self.detail.tags.push(tag);
                let Some(ref fid) = self.detail.file_id else { return Task::none(); };
                let fid = fid.clone();
                let tags = self.detail.tags.clone();
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap();
                        let _ = db::upsert_tags(&g, &fid, &tags);
                    },
                    |()| Msg::NoOp,
                )
            }

            Msg::RemoveDetailTag(tag) => {
                self.detail.tags.retain(|t| t != &tag);
                let Some(ref fid) = self.detail.file_id else { return Task::none(); };
                let fid = fid.clone();
                let tags = self.detail.tags.clone();
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap();
                        let _ = db::upsert_tags(&g, &fid, &tags);
                    },
                    |()| Msg::NoOp,
                )
            }

            Msg::SetDetailRating(n) => {
                let new_rating = if self.detail.rating == Some(n) { None } else { Some(n) };
                self.detail.rating = new_rating;
                let Some(ref fid) = self.detail.file_id else { return Task::none(); };
                let fid = fid.clone();
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                Task::perform(
                    async move {
                        let g = conn.lock().unwrap();
                        let _ = db::set_file_rating(&g, &fid, new_rating);
                    },
                    |()| Msg::NoOp,
                )
            }

            Msg::Reload => {
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::Tick => {
                // Drain thumbnails
                while let Ok(ev) = self.thumbnail_rx.try_recv() {
                    match ev {
                        ThumbnailEvent::Ready(fid, path) => {
                            self.thumbnails.insert(fid, ThumbnailState::Ready(path));
                        }
                        ThumbnailEvent::Failed(fid, _err) => {
                            self.thumbnails.insert(fid, ThumbnailState::Failed(0));
                        }
                    }
                }
                let mut tasks: Vec<Task<Msg>> = Vec::new();

                // Flush search debounce after 300ms idle
                if let Some((_, ts)) = &self.pending_search {
                    if ts.elapsed() >= Duration::from_millis(300) {
                        let (text, _) = self.pending_search.take().unwrap();
                        self.search_text = text;
                        self.scroll_y = 0.0;
                        self.files.clear();
                        self.grid_selected.clear();
                        tasks.push(self.load_files_task());
                    }
                }

                // Drain file watcher events
                let mut file_events: Vec<FileEvent> = Vec::new();
                while let Ok(ev) = self.watcher_rx.try_recv() {
                    match ev {
                        FileEvent::ScanProgress(prog) => {
                            self.status = format!("Scanning {}… {} found", prog.folder_name, prog.total_found);
                        }
                        other => file_events.push(other),
                    }
                }
                if !file_events.is_empty() {
                    if let Some(conn) = self.conn.clone() {
                        tasks.push(Task::perform(
                            async move {
                                let guard = conn.lock().unwrap();
                                for event in file_events {
                                    match event {
                                        FileEvent::Created(path) | FileEvent::Modified(path) => {
                                            let _ = scanner::resync_files(&guard, &[path]);
                                        }
                                        FileEvent::Deleted(path) => {
                                            let norm = normalize_path(&path);
                                            let fid = compute_file_id(&norm);
                                            let _ = db::mark_orphaned(&guard, &fid);
                                        }
                                        FileEvent::Renamed { old_path, new_path } => {
                                            let norm = normalize_path(&old_path);
                                            let old_fid = compute_file_id(&norm);
                                            let _ = db::mark_orphaned(&guard, &old_fid);
                                            let _ = scanner::resync_files(&guard, &[new_path]);
                                        }
                                        FileEvent::SidecarChanged(path) => {
                                            let _ = scanner::resync_sidecar_files(&guard, &[path]);
                                        }
                                        FileEvent::SidecarRemoved(_) => {}
                                        FileEvent::ScanProgress(_) => {}
                                    }
                                }
                            },
                            |()| Msg::Reload,
                        ));
                    }
                }

                Task::batch(tasks)
            }

            Msg::DbError(e) => {
                self.status = format!("Error: {e}");
                Task::none()
            }

            Msg::DragHoverAlbum(opt_id) => {
                if self.drag.as_ref().map_or(false, |d| d.active) {
                    self.drag_hover_album = opt_id;
                }
                Task::none()
            }

            Msg::PickOpenCatalog => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Open Catalog")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_string_lossy().to_string())
                },
                |opt| match opt {
                    Some(path) => Msg::OpenCatalogPicked(path),
                    None => Msg::NoOp,
                },
            ),

            Msg::OpenCatalogPicked(path) => {
                Task::done(Msg::OpenCatalog(path))
            }

            Msg::PickNewCatalogDir => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose location for new catalog")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_string_lossy().to_string())
                },
                |opt| match opt {
                    Some(dir) => Msg::NewCatalogDirPicked(dir),
                    None => Msg::NoOp,
                },
            ),

            Msg::NewCatalogDirPicked(dir) => {
                self.new_catalog_dir = Some(dir);
                self.new_catalog_name.clear();
                Task::none()
            }

            Msg::NewCatalogNameChanged(s) => {
                self.new_catalog_name = s;
                Task::none()
            }

            Msg::ConfirmNewCatalog => {
                let Some(dir) = self.new_catalog_dir.take() else { return Task::none(); };
                let name = self.new_catalog_name.trim().to_string();
                if name.is_empty() {
                    self.new_catalog_dir = Some(dir);
                    return Task::none();
                }
                Task::perform(
                    async move {
                        isomfolio_core::app_paths::create_catalog(&dir, &name)
                            .map_err(|e| e.to_string())
                    },
                    |result| match result {
                        Ok(path) => Msg::OpenCatalog(path),
                        Err(e) => Msg::DbError(e),
                    },
                )
            }

            Msg::OpenCatalog(path) => {
                isomfolio_core::app_paths::save_recent_catalog(&path);
                self.watchers.clear();
                self.thumbnail_pool = None;
                self.files.clear();
                self.thumbnails.clear();
                self.folders.clear();
                self.albums.clear();
                self.album_counts.clear();
                self.grid_selected.clear();
                self.drag = None;
                self.dragging_ids.clear();
                self.pending_search = None;
                self.search_text.clear();
                self.criteria = CriteriaState::default();
                self.detail = DetailState::default();
                self.selected_item = SidebarItem::AllFiles;
                self.scroll_y = 0.0;
                self.loupe_idx = 0;
                self.view_mode = ViewMode::Browse;
                self.album_pending_delete = None;
                self.folder_pending_remove = None;
                isomfolio_core::app_paths::ensure_directories(&path);
                self.conn = db::open_database(&db_path(&path))
                    .ok()
                    .map(|c| Arc::new(Mutex::new(c)));
                self.status = if self.conn.is_none() {
                    "Error: could not open database — check permissions".to_string()
                } else {
                    String::new()
                };
                self.catalog_dir = path;
                self.show_welcome = false;
                self.recent_catalogs = isomfolio_core::app_paths::read_recent_catalogs();
                Task::done(Msg::CatalogReady)
            }

            Msg::NoOp => Task::none(),
        }
    }

    pub fn subscription(&self) -> Subscription<Msg> {
        let tick_sub = iced::time::every(std::time::Duration::from_millis(50))
            .map(|_| Msg::Tick);

        let event_sub = event::listen_with(|event, _status, _id| match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                Some(Msg::MouseMoved(position))
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                Some(Msg::MousePressed)
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                Some(Msg::MouseReleased)
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => {
                Some(Msg::ModifiersChanged(m))
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => Some(Msg::EscapePressed),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Enter),
                ..
            }) => Some(Msg::OpenLoupe),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Character(ref c),
                ..
            }) if c.as_str() == "i" => Some(Msg::ToggleDetail),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::ArrowLeft),
                ..
            }) => Some(Msg::Navigate { dx: -1, dy: 0 }),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::ArrowRight),
                ..
            }) => Some(Msg::Navigate { dx: 1, dy: 0 }),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::ArrowUp),
                ..
            }) => Some(Msg::Navigate { dx: 0, dy: -1 }),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::ArrowDown),
                ..
            }) => Some(Msg::Navigate { dx: 0, dy: 1 }),
            _ => None,
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

fn next_sort_field(f: SortField) -> SortField {
    match f {
        SortField::Name => SortField::Date,
        SortField::Date => SortField::Size,
        SortField::Size => SortField::Ext,
        SortField::Ext => SortField::Name,
    }
}

fn new_album_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("alb-{nanos:x}-{seq:x}")
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
