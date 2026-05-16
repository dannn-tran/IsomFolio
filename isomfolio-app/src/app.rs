use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};

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
    NoOp,
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
    pub thumbnail_rx: Arc<Mutex<mpsc::Receiver<ThumbnailEvent>>>,

    pub watcher_tx: mpsc::SyncSender<FileEvent>,
    pub watcher_rx: Arc<Mutex<mpsc::Receiver<FileEvent>>>,
    pub watchers: Vec<(String, FileWatcher)>,

    pub scan_count: Arc<AtomicUsize>,
    pub scan_folder_name: String,

    pub search_text: String,
    pub create_album_input: Option<String>,
    pub rename_album_id: Option<AlbumId>,
    pub rename_album_input: String,

    pub sort_by: SortField,
    pub sort_asc: bool,

    // Criteria panel
    pub show_criteria: bool,
    pub criteria_tags: Vec<String>,
    pub criteria_tag_input: String,
    pub criteria_date_from: String,
    pub criteria_date_to: String,
    pub criteria_exts: HashSet<String>,
    pub save_smart_input: Option<String>,

    // Detail panel
    pub show_detail: bool,
    pub detail_file_id: Option<String>,
    pub detail_tags: Vec<String>,
    pub detail_tag_input: String,
    pub detail_rating: Option<i32>,
    pub detail_label: Option<String>,
    pub detail_title: Option<String>,

    pub status: String,
    pub is_scanning: bool,
}

impl App {
    pub fn new(catalog_dir: String) -> (Self, Task<Msg>) {
        let (tx, rx) = mpsc::sync_channel::<ThumbnailEvent>(500);
        let (wtx, wrx) = mpsc::sync_channel::<FileEvent>(200);

        let conn = isomfolio_core::app_paths::ensure_directories(&catalog_dir);
        let _ = conn;
        let conn = db::open_database(&db_path(&catalog_dir))
            .ok()
            .map(|c| Arc::new(Mutex::new(c)));

        let initial_status = if conn.is_none() {
            "Error: could not open database — check permissions".to_string()
        } else {
            String::new()
        };

        let app = App {
            conn,
            catalog_dir,
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
            thumbnail_rx: Arc::new(Mutex::new(rx)),
            watcher_tx: wtx,
            watcher_rx: Arc::new(Mutex::new(wrx)),
            watchers: Vec::new(),
            scan_count: Arc::new(AtomicUsize::new(0)),
            scan_folder_name: String::new(),
            search_text: String::new(),
            create_album_input: None,
            rename_album_id: None,
            rename_album_input: String::new(),
            sort_by: SortField::Name,
            sort_asc: true,
            show_criteria: false,
            criteria_tags: Vec::new(),
            criteria_tag_input: String::new(),
            criteria_date_from: String::new(),
            criteria_date_to: String::new(),
            criteria_exts: HashSet::new(),
            save_smart_input: None,
            show_detail: false,
            detail_file_id: None,
            detail_tags: Vec::new(),
            detail_tag_input: String::new(),
            detail_rating: None,
            detail_label: None,
            detail_title: None,
            status: initial_status,
            is_scanning: false,
        };

        let task = Task::done(Msg::CatalogReady);
        (app, task)
    }

    pub fn cols(&self) -> usize {
        let detail_w = if self.show_detail { SIDEBAR_WIDTH } else { 0.0 };
        let avail = (self.viewport_width - 2.0 * GRID_PADDING - detail_w).max(0.0);
        ((avail + TILE_GAP) / (self.tile_px + TILE_GAP)) as usize
    }

    pub fn criteria_panel_height(&self) -> f32 {
        if !self.show_criteria {
            return 0.0;
        }
        let rows = CRITERIA_ROW_COUNT as f32;
        let spacing = (CRITERIA_ROW_COUNT - 1) as f32 * 6.0;
        let action_row = if self.criteria_has_any() { CRITERIA_ROW_HEIGHT + 6.0 } else { 0.0 };
        rows * CRITERIA_ROW_HEIGHT + spacing + CRITERIA_PADDING + action_row
    }

    pub fn criteria_has_any(&self) -> bool {
        !self.criteria_tags.is_empty()
            || !self.criteria_exts.is_empty()
            || !self.criteria_date_from.is_empty()
            || !self.criteria_date_to.is_empty()
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
        let id = self.detail_file_id.as_deref()?;
        self.files.iter().find(|f| f.id == id)
    }

    pub fn build_search_query(&self) -> SearchQuery {
        let text_opt = {
            let t = self.search_text.trim();
            if t.is_empty() { None } else { Some(t.to_string()) }
        };
        SearchQuery {
            text: text_opt,
            tags: self.criteria_tags.clone(),
            extensions: self.criteria_exts.iter().cloned().collect(),
            date_from: parse_date_str(&self.criteria_date_from),
            date_to: parse_date_str(&self.criteria_date_to),
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
        if !self.show_detail {
            return Task::none();
        }
        if self.grid_selected.len() != 1 {
            return Task::none();
        }
        let file_id = self.grid_selected.iter().next().unwrap().clone();
        if self.detail_file_id.as_deref() == Some(file_id.as_str()) {
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

    pub fn album_section_top(&self) -> f32 {
        let base = 102.0_f32;
        if self.folders.is_empty() {
            base
        } else {
            base + 27.0 + self.folders.len() as f32 * (ALBUM_ITEM_HEIGHT + 2.0)
        }
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

    pub fn album_at(&self, pos: Point) -> Option<AlbumId> {
        if pos.x > SIDEBAR_WIDTH {
            return None;
        }
        let top = self.album_section_top();
        let rel_y = pos.y - top;
        if rel_y < 0.0 {
            return None;
        }
        let idx = (rel_y / (ALBUM_ITEM_HEIGHT + 2.0)) as usize;
        self.albums.get(idx).map(|a| a.id.clone())
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
                            self.criteria_tags = q.tags.clone();
                            self.criteria_date_from =
                                q.date_from.map(unix_to_date_str).unwrap_or_default();
                            self.criteria_date_to =
                                q.date_to.map(unix_to_date_str).unwrap_or_default();
                            self.criteria_exts = q.extensions.iter().cloned().collect();
                            self.search_text = q.text.clone().unwrap_or_default();
                            self.show_criteria = true;
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
                self.save_smart_input = None;
                self.detail_file_id = None;
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
                    if let Some(ref d) = self.drag {
                        self.drag_hover_album = if d.active {
                            self.album_at(d.cursor)
                        } else {
                            None
                        };
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
                    }
                }
                if self.show_detail && self.grid_selected.len() != 1 {
                    self.detail_file_id = None;
                    self.detail_tags.clear();
                    self.detail_rating = None;
                    self.detail_label = None;
                    self.detail_title = None;
                }
                Task::none()
            }

            Msg::MouseReleased => {
                let drop_task = if let Some(ref d) = self.drag {
                    if d.active {
                        if let Some(album_id) = self.album_at(d.cursor) {
                            Some(Task::done(Msg::DroppedToAlbum(album_id)))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
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
                self.save_smart_input = None;
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

            Msg::ScanPickFolder => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose folder to scan")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_string_lossy().to_string())
                },
                |opt| match opt {
                    Some(path) => Msg::ScanStart(path),
                    None => Msg::NoOp,
                },
            ),

            Msg::ScanStart(path) => {
                self.is_scanning = true;
                self.scan_folder_name = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&path)
                    .to_string();
                self.scan_count.store(0, Ordering::Relaxed);
                self.status = format!("Scanning {}…", self.scan_folder_name);
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                let count = Arc::clone(&self.scan_count);
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let guard = conn.lock().unwrap();
                            scanner::scan_folder(&guard, &path, &|_| {}, &|prog| {
                                count.store(prog.total_found, Ordering::Relaxed);
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

            Msg::RemoveFolder(path) => {
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

            Msg::DeleteAlbum(album_id) => {
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

            Msg::SearchChanged(text) => {
                self.search_text = text;
                self.scroll_y = 0.0;
                self.files.clear();
                self.grid_selected.clear();
                self.load_files_task()
            }

            // Criteria panel

            Msg::ToggleCriteria => {
                self.show_criteria = !self.show_criteria;
                Task::none()
            }

            Msg::CriteriaTagInputChanged(s) => {
                self.criteria_tag_input = s;
                Task::none()
            }

            Msg::AddCriteriaTag => {
                let tag = self.criteria_tag_input.trim().to_string();
                self.criteria_tag_input.clear();
                if !tag.is_empty() && !self.criteria_tags.contains(&tag) {
                    self.criteria_tags.push(tag);
                }
                self.load_files_task()
            }

            Msg::RemoveCriteriaTag(tag) => {
                self.criteria_tags.retain(|t| t != &tag);
                self.load_files_task()
            }

            Msg::CriteriaDateFromChanged(s) => {
                self.criteria_date_from = s;
                self.load_files_task()
            }

            Msg::CriteriaDateToChanged(s) => {
                self.criteria_date_to = s;
                self.load_files_task()
            }

            Msg::ToggleCriteriaExt(ext) => {
                if self.criteria_exts.contains(&ext) {
                    self.criteria_exts.remove(&ext);
                } else {
                    self.criteria_exts.insert(ext);
                }
                self.load_files_task()
            }

            Msg::ClearCriteria => {
                self.criteria_tags.clear();
                self.criteria_date_from.clear();
                self.criteria_date_to.clear();
                self.criteria_exts.clear();
                self.load_files_task()
            }

            // Smart album handlers

            Msg::SaveAsSmartAlbum => {
                self.save_smart_input = Some(String::new());
                Task::none()
            }

            Msg::SmartAlbumNameChanged(s) => {
                self.save_smart_input = Some(s);
                Task::none()
            }

            Msg::ConfirmSmartAlbum => {
                let name = self.save_smart_input.take().unwrap_or_default();
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
                self.show_detail = !self.show_detail;
                if self.show_detail {
                    self.detail_file_id = None;
                    self.maybe_load_detail()
                } else {
                    Task::none()
                }
            }

            Msg::DetailLoaded { file_id, tags, rating, label, title } => {
                self.detail_file_id = Some(file_id);
                self.detail_tags = tags;
                self.detail_rating = rating;
                self.detail_label = label;
                self.detail_title = title;
                Task::none()
            }

            Msg::DetailTagInputChanged(s) => {
                self.detail_tag_input = s;
                Task::none()
            }

            Msg::AddDetailTag => {
                let tag = self.detail_tag_input.trim().to_string();
                self.detail_tag_input.clear();
                if tag.is_empty() || self.detail_tags.contains(&tag) {
                    return Task::none();
                }
                self.detail_tags.push(tag);
                let Some(ref fid) = self.detail_file_id else { return Task::none(); };
                let fid = fid.clone();
                let tags = self.detail_tags.clone();
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
                self.detail_tags.retain(|t| t != &tag);
                let Some(ref fid) = self.detail_file_id else { return Task::none(); };
                let fid = fid.clone();
                let tags = self.detail_tags.clone();
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
                let new_rating = if self.detail_rating == Some(n) { None } else { Some(n) };
                self.detail_rating = new_rating;
                let Some(ref fid) = self.detail_file_id else { return Task::none(); };
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
                if let Ok(rx) = self.thumbnail_rx.lock() {
                    while let Ok(ev) = rx.try_recv() {
                        match ev {
                            ThumbnailEvent::Ready(fid, path) => {
                                self.thumbnails.insert(fid, ThumbnailState::Ready(path));
                            }
                            ThumbnailEvent::Failed(fid, _err) => {
                                self.thumbnails.insert(fid, ThumbnailState::Failed(0));
                            }
                        }
                    }
                }
                // Update scan progress
                if self.is_scanning {
                    let n = self.scan_count.load(Ordering::Relaxed);
                    if n > 0 {
                        self.status = format!("Scanning {}… {} found", self.scan_folder_name, n);
                    }
                }
                // Drain file watcher events
                let mut file_events: Vec<FileEvent> = Vec::new();
                if let Ok(rx) = self.watcher_rx.lock() {
                    while let Ok(ev) = rx.try_recv() {
                        file_events.push(ev);
                    }
                }
                if file_events.is_empty() {
                    return Task::none();
                }
                let Some(conn) = self.conn.clone() else { return Task::none(); };
                Task::perform(
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
                            }
                        }
                    },
                    |()| Msg::Reload,
                )
            }

            Msg::DbError(e) => {
                self.status = format!("Error: {e}");
                Task::none()
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
