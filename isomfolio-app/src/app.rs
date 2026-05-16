use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, mpsc};

use iced::{
    Event, Point, Subscription, Task,
    event, keyboard, mouse,
};

use isomfolio_core::models::{Album, AlbumId, AssetFile, ThumbnailState};
use isomfolio_core::storage::db;
use isomfolio_core::app_paths::db_path;
use isomfolio_core::indexing::thumbnail::{ThumbnailPool, create_worker_pool, thumbnail_cache_path};
use isomfolio_core::Connection;
use isomfolio_core::search::query_engine::{execute_search, execute_manual_album_search};
use isomfolio_core::models::SearchQuery;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const SIDEBAR_WIDTH: f32 = 220.0;
pub const GRID_PADDING: f32 = 12.0;
pub const TILE_GAP: f32 = 8.0;
pub const ALBUM_TOP_OFFSET: f32 = 62.0; // sidebar header + spacing
pub const ALBUM_ITEM_HEIGHT: f32 = 44.0;
pub const DRAG_THRESHOLD: f32 = 6.0;
pub const BUFFER_ROWS: usize = 2;

// ---------------------------------------------------------------------------
// Sidebar selection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum SidebarItem {
    AllFiles,
    Folder(String),
    Album(AlbumId),
}

// ---------------------------------------------------------------------------
// Drag state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DragState {
    pub origin_idx: usize,
    pub start: Point,
    pub cursor: Point,
    pub active: bool,
}

// ---------------------------------------------------------------------------
// Thumbnail channel messages
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ThumbnailEvent {
    Ready(String, String),
    Failed(String, String),
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Msg {
    // Lifecycle
    CatalogReady,

    // Sidebar navigation
    SidebarItemClicked(SidebarItem),

    // Data loaded
    FilesLoaded(Vec<AssetFile>),
    AlbumsLoaded(Vec<Album>),
    FolderCountsLoaded(Vec<(String, usize)>),

    // Tile size
    TileSizeUp,
    TileSizeDown,

    // Grid interaction (global subscription)
    MouseMoved(Point),
    MousePressed,
    MouseReleased,
    ModifiersChanged(keyboard::Modifiers),

    // Scroll
    Scrolled { y: f32, height: f32, width: f32 },

    // Album drop
    DroppedToAlbum(AlbumId),
    DropCompleted,

    // DB error
    DbError(String),

    Tick,
    NoOp,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct App {
    // Catalog
    pub conn: Option<Arc<Mutex<Connection>>>,
    pub catalog_dir: String,

    // Sidebar
    pub folders: Vec<(String, usize)>,
    pub albums: Vec<Album>,
    pub selected_item: SidebarItem,
    pub drag_hover_album: Option<AlbumId>,

    // Grid
    pub files: Vec<AssetFile>,
    pub thumbnails: HashMap<String, ThumbnailState>,
    pub grid_selected: HashSet<String>,
    pub tile_px: f32,
    pub anchor_idx: Option<usize>,

    // Virtual scroll
    pub scroll_y: f32,
    pub viewport_height: f32,
    pub viewport_width: f32,

    // Drag
    pub cursor: Point,
    pub drag: Option<DragState>,
    pub dragging_ids: HashSet<String>,
    pub modifiers: keyboard::Modifiers,

    // Background workers
    pub thumbnail_pool: Option<ThumbnailPool>,
    pub thumbnail_tx: mpsc::SyncSender<ThumbnailEvent>,
    pub thumbnail_rx: Arc<Mutex<mpsc::Receiver<ThumbnailEvent>>>,

    pub status: String,
}

impl App {
    pub fn new(catalog_dir: String) -> (Self, Task<Msg>) {
        let (tx, rx) = mpsc::sync_channel::<ThumbnailEvent>(500);

        let conn = isomfolio_core::app_paths::ensure_directories(&catalog_dir);
        let _ = conn;
        let conn = db::open_database(&db_path(&catalog_dir))
            .ok()
            .map(|c| Arc::new(Mutex::new(c)));

        let app = App {
            conn,
            catalog_dir,
            folders: Vec::new(),
            albums: Vec::new(),
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
            status: String::new(),
        };

        let task = Task::done(Msg::CatalogReady);
        (app, task)
    }

    pub fn cols(&self) -> usize {
        let avail = (self.viewport_width - 2.0 * GRID_PADDING).max(0.0);
        ((avail + TILE_GAP) / (self.tile_px + TILE_GAP)) as usize
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

    fn load_files_task(&self) -> Task<Msg> {
        let Some(conn) = self.conn.clone() else {
            return Task::done(Msg::FilesLoaded(Vec::new()));
        };
        let item = self.selected_item.clone();
        Task::perform(
            async move {
                let guard = conn.lock().unwrap();
                match item {
                    SidebarItem::AllFiles => {
                        let q = SearchQuery::default();
                        execute_search(&guard, &q).unwrap_or_default()
                    }
                    SidebarItem::Folder(path) => {
                        db::get_files_by_folder_recursive(&guard, &path).unwrap_or_default()
                    }
                    SidebarItem::Album(album_id) => {
                        execute_manual_album_search(&guard, &album_id).unwrap_or_default()
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
        let conn2 = Arc::clone(&conn);
        let t1 = Task::perform(
            async move {
                let g = conn.lock().unwrap();
                db::get_folder_counts(&g).unwrap_or_default()
            },
            Msg::FolderCountsLoaded,
        );
        let t2 = Task::perform(
            async move {
                let g = conn2.lock().unwrap();
                db::get_all_albums(&g).unwrap_or_default()
            },
            Msg::AlbumsLoaded,
        );
        Task::batch([t1, t2])
    }

    // ---------------------------------------------------------------------------
    // Hit testing
    // ---------------------------------------------------------------------------

    pub fn tile_index_at(&self, pos: Point) -> Option<usize> {
        let rel_x = pos.x - SIDEBAR_WIDTH - GRID_PADDING;
        // Convert window y to scroll-content y
        let rel_y = pos.y + self.scroll_y - GRID_PADDING;
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
        let rel_y = pos.y - ALBUM_TOP_OFFSET;
        if rel_y < 0.0 {
            return None;
        }
        let idx = (rel_y / ALBUM_ITEM_HEIGHT) as usize;
        self.albums.get(idx).map(|a| a.id.clone())
    }

    // ---------------------------------------------------------------------------
    // Update
    // ---------------------------------------------------------------------------

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::CatalogReady => {
                self.start_thumbnail_pool();
                let t1 = self.load_sidebar_task();
                let t2 = self.load_files_task();
                Task::batch([t1, t2])
            }

            Msg::SidebarItemClicked(item) => {
                self.selected_item = item;
                self.files.clear();
                self.scroll_y = 0.0;
                self.grid_selected.clear();
                self.drag = None;
                self.dragging_ids.clear();
                self.load_files_task()
            }

            Msg::FilesLoaded(files) => {
                self.files = files;
                self.enqueue_thumbnails();
                self.status = format!("{} photo(s)", self.files.len());
                Task::none()
            }

            Msg::AlbumsLoaded(albums) => {
                self.albums = albums;
                Task::none()
            }

            Msg::FolderCountsLoaded(counts) => {
                self.folders = counts;
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
                Task::none()
            }

            Msg::MouseReleased => {
                if let Some(ref d) = self.drag {
                    if d.active {
                        if let Some(album_id) = self.album_at(d.cursor) {
                            return Task::done(Msg::DroppedToAlbum(album_id));
                        }
                    }
                }
                self.drag = None;
                self.dragging_ids.clear();
                self.drag_hover_album = None;
                Task::none()
            }

            Msg::ModifiersChanged(m) => {
                self.modifiers = m;
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

            Msg::DropCompleted => Task::none(),

            Msg::Tick => {
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
                Task::none()
            }

            Msg::DbError(e) => {
                self.status = format!("Error: {e}");
                Task::none()
            }

            Msg::NoOp => Task::none(),
        }
    }

    // ---------------------------------------------------------------------------
    // Subscription
    // ---------------------------------------------------------------------------

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
            _ => None,
        });

        Subscription::batch([tick_sub, event_sub])
    }
}
