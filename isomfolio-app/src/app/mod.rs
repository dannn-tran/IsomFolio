pub mod keybinds;
mod types;
mod update;

pub use types::*;

use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

use iced::{event, keyboard, mouse, Event, Point, Size, Subscription, Task};

use isomfolio_core::extension::ExtensionProcess;
use isomfolio_core::app_paths::db_path;
use isomfolio_core::Catalog;
use isomfolio_core::indexing::thumbnail::{
    create_worker_pool, thumbnail_cache_path, ThumbnailPool,
};
use isomfolio_core::indexing::types::FileEvent;
use isomfolio_core::indexing::watcher::{create_watcher, FileWatcher};
use isomfolio_core::models::SearchQuery;
use isomfolio_core::models::{Album, AlbumId, AlbumKind, AssetFile, SortField, ThumbnailState};

pub trait LockUnwrap<T> {
    fn lock_unwrap(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> LockUnwrap<T> for Mutex<T> {
    fn lock_unwrap(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub struct LoupeState {
    pub idx: usize,
    pub full_res: Option<(usize, iced::widget::image::Handle)>,
    pub prefetch: HashMap<usize, iced::widget::image::Handle>,
}

impl Default for LoupeState {
    fn default() -> Self {
        Self { idx: 0, full_res: None, prefetch: HashMap::new() }
    }
}

pub struct ThumbnailContext {
    pub pool: Option<ThumbnailPool>,
    pub tx: mpsc::SyncSender<ThumbnailEvent>,
    pub rx: Arc<Mutex<Option<mpsc::Receiver<ThumbnailEvent>>>>,
    pub sub_id: u64,
    pub pending: usize,
    pub total: usize,
    pub start_at: Option<Instant>,
    pub done_gen: u64,
    pub handles: HashMap<String, iced::widget::image::Handle>,
}

pub struct DragContext {
    pub state: Option<DragState>,
    pub ids: HashSet<String>,
    pub hover_album: Option<AlbumId>,
}

impl Default for DragContext {
    fn default() -> Self {
        Self { state: None, ids: HashSet::new(), hover_album: None }
    }
}

pub struct WelcomeState {
    pub show: bool,
    pub recent_catalogs: Vec<String>,
    pub selected_recent_catalog: Option<String>,
    pub show_new_catalog_modal: bool,
    pub new_catalog_dir: Option<std::path::PathBuf>,
    pub new_catalog_name: String,
}

pub struct FaceState {
    pub clusters: Vec<isomfolio_core::models::FaceClusterSummary>,
    pub crop_handles: HashMap<String, iced::widget::image::Handle>,
    pub rename_cluster_id: Option<String>,
    pub rename_input: String,
    pub status: Option<String>,
    pub is_clustering: bool,
    /// Embedding progress 0.0–1.0 while clustering; `None` = indeterminate
    /// (engine starting / model download).
    pub progress: Option<f32>,
}

impl Default for FaceState {
    fn default() -> Self {
        Self { clusters: Vec::new(), crop_handles: HashMap::new(), rename_cluster_id: None, rename_input: String::new(), status: None, is_clustering: false, progress: None }
    }
}

#[derive(Debug, Clone)]
pub struct MetadataImportPrompt {
    pub pending_path: String,
    pub import_xmp: bool,
    pub import_apple: bool,
}

pub struct App {
    pub catalog: Option<Arc<Mutex<Catalog>>>,
    pub catalog_dir: String,

    pub view_mode: ViewMode,
    pub loupe: LoupeState,

    pub folders: Vec<(String, String, usize)>,
    pub albums: Vec<Album>,
    pub album_counts: HashMap<String, usize>,
    pub selected_item: SidebarItem,

    pub files: Vec<AssetFile>,
    pub file_ratings: HashMap<String, i32>,
    pub pending_counts_by_id: HashMap<String, usize>,
    pub pending_tag_file_count: usize,
    pub suggestion_view: SuggestionView,
    pub pending_tag_groups: Vec<isomfolio_core::models::PendingTagGroup>,
    pub thumbnails: HashMap<String, ThumbnailState>,
    pub grid_selected: HashSet<String>,
    pub tile_px: f32,
    pub anchor_idx: Option<usize>,

    pub scroll_y: f32,
    pub viewport_height: f32,
    pub viewport_width: f32,

    pub cursor: Point,
    pub drag: DragContext,
    pub modifiers: keyboard::Modifiers,

    pub thumb_ctx: ThumbnailContext,

    pub watcher_tx: mpsc::SyncSender<FileEvent>,
    pub watcher_rx: Arc<std::sync::Mutex<Option<mpsc::Receiver<FileEvent>>>>,
    pub watchers: Vec<(String, FileWatcher)>,
    pub pending_file_events: Vec<FileEvent>,
    pub watcher_debounce_id: u64,

    pub search_text: String,
    pub search_debounce_id: u64,
    pub create_album_input: Option<String>,
    pub rename_album_id: Option<AlbumId>,
    pub rename_album_input: String,

    pub sort_by: SortField,
    pub sort_asc: bool,

    pub filters: FilterState,
    pub detail: DetailState,

    pub show_shortcut_help: bool,
    pub open_menu: Option<String>,
    pub status: String,
    pub is_syncing: bool,
    pub sync_pending: bool,

    pub welcome: WelcomeState,
    pub album_pending_delete: Option<AlbumId>,
    pub folder_pending_remove: Option<String>,
    pub remove_missing_folder: Option<String>,
    pub metadata_import_prompt: Option<MetadataImportPrompt>,
    pub sidebar_scroll_y: f32,

    pub last_click_time: Option<Instant>,
    pub pending_album_select: Option<AlbumId>,
    pub last_synced_path: Option<String>,
    pub remove_from_album_pending: bool,
    pub smart_album_dirty: bool,
    pub context_menu: Option<ContextMenuState>,
    pub hovered_sidebar_entity: Option<SidebarItem>,
    pub tag_browser: Option<TagBrowserState>,

    pub sidebar_width: f32,
    pub sidebar_resizing: bool,

    pub extensions: Vec<Arc<ExtensionProcess>>,
    pub settings: SettingsState,
    pub app_settings: isomfolio_core::app_paths::AppSettings,

    pub faces: FaceState,
    /// Lazily-spawned local inference engine (or remote client). Held for the
    /// session and dropped — killing any managed child — when the app quits.
    pub inference: Option<Arc<crate::inference::InferenceClient>>,
    /// Manifest of the installed inference-engine extension, if any. Discovered
    /// but never IEP-launched; provides the binary path for managed launch and
    /// gates the "Find people" UI.
    pub inference_manifest: Option<isomfolio_core::extension::ExtensionManifest>,

    pub undo_stack: Vec<UndoOp>,
    pub redo_stack: Vec<UndoOp>,

    pub compare: CompareState,

    pub bg_tasks: Vec<crate::app::types::BgTask>,
    pub next_bg_task_id: crate::app::types::BgTaskId,
    pub task_panel_open: bool,
}

pub struct CompareState {
    pub files: [Option<isomfolio_core::models::AssetFile>; 2],
    pub handles: [Option<iced::widget::image::Handle>; 2],
}

impl Default for CompareState {
    fn default() -> Self {
        Self { files: [None, None], handles: [None, None] }
    }
}

struct ThumbnailRecipe {
    rx: Arc<std::sync::Mutex<Option<mpsc::Receiver<ThumbnailEvent>>>>,
    id: u64,
}

impl iced::advanced::subscription::Recipe for ThumbnailRecipe {
    type Output = Msg;

    fn hash(&self, state: &mut iced::advanced::subscription::Hasher) {
        use std::hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
        self.id.hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: iced::advanced::subscription::EventStream,
    ) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = Self::Output> + Send + 'static>> {
        let rx_arc = self.rx;
        Box::pin(iced::futures::stream::unfold(
            None::<mpsc::Receiver<ThumbnailEvent>>,
            move |rx| {
                let rx_arc = rx_arc.clone();
                async move {
                    let rx = match rx {
                        Some(r) => r,
                        None => rx_arc.lock().ok()?.take()?,
                    };
                    let result = tokio::task::spawn_blocking(move || {
                        rx.recv().ok().map(|ev| (ev, rx))
                    })
                    .await
                    .ok()
                    .flatten()?;
                    let (event, rx) = result;
                    let msg = match event {
                        ThumbnailEvent::Ready(fid, path) => Msg::ThumbnailCompleted { file_id: fid, path },
                        ThumbnailEvent::Failed(fid) => Msg::ThumbnailFailed { file_id: fid },
                    };
                    Some((msg, Some(rx)))
                }
            },
        ))
    }
}

struct WatcherRecipe {
    rx: Arc<std::sync::Mutex<Option<mpsc::Receiver<FileEvent>>>>,
}

impl iced::advanced::subscription::Recipe for WatcherRecipe {
    type Output = Msg;

    fn hash(&self, state: &mut iced::advanced::subscription::Hasher) {
        use std::hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: iced::advanced::subscription::EventStream,
    ) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = Self::Output> + Send + 'static>> {
        let rx_arc = self.rx;
        Box::pin(iced::futures::stream::unfold(
            None::<mpsc::Receiver<FileEvent>>,
            move |rx| {
                let rx_arc = rx_arc.clone();
                async move {
                    let rx = match rx {
                        Some(r) => r,
                        None => rx_arc.lock().ok()?.take()?,
                    };
                    let result = tokio::task::spawn_blocking(move || {
                        rx.recv().ok().map(|ev| (ev, rx))
                    })
                    .await
                    .ok()
                    .flatten()?;
                    let (event, rx) = result;
                    Some((Msg::FileWatcherEvent(event), Some(rx)))
                }
            },
        ))
    }
}

impl App {
    pub fn new(catalog_dir: Option<String>) -> (Self, Task<Msg>) {
        let (tx, rx) = mpsc::sync_channel::<ThumbnailEvent>(500);
        let rx_arc = Arc::new(std::sync::Mutex::new(Some(rx)));
        let (wtx, wrx) = mpsc::sync_channel::<FileEvent>(200);
        let wrx_arc = Arc::new(std::sync::Mutex::new(Some(wrx)));

        let recent_catalogs = isomfolio_core::app_paths::read_recent_catalogs();

        let (catalog_dir_str, catalog, initial_status, show_welcome, task) = match catalog_dir {
            Some(dir) => {
                isomfolio_core::app_paths::ensure_directories(&dir);
                let catalog = Catalog::open(&db_path(&dir))
                    .ok()
                    .map(|c| Arc::new(Mutex::new(c)));
                let status = if catalog.is_none() {
                    "Error: could not open database — check permissions".to_string()
                } else {
                    String::new()
                };
                (dir, catalog, status, false, Task::done(Msg::CatalogReady))
            }
            None => (String::new(), None, String::new(), true, Task::none()),
        };

        let app = App {
            catalog,
            catalog_dir: catalog_dir_str,
            view_mode: ViewMode::Browse,
            loupe: LoupeState::default(),
            folders: Vec::new(),
            albums: Vec::new(),
            album_counts: HashMap::new(),
            selected_item: SidebarItem::AllFiles,
            files: Vec::new(),
            file_ratings: HashMap::new(),
            pending_counts_by_id: HashMap::new(),
            pending_tag_file_count: 0,
            suggestion_view: SuggestionView::Photo,
            pending_tag_groups: Vec::new(),
            thumbnails: HashMap::new(),
            grid_selected: HashSet::new(),
            tile_px: 180.0,
            anchor_idx: None,
            scroll_y: 0.0,
            viewport_height: 600.0,
            viewport_width: 1060.0,
            cursor: Point::ORIGIN,
            drag: DragContext::default(),
            modifiers: keyboard::Modifiers::default(),
            thumb_ctx: ThumbnailContext {
                pool: None,
                tx,
                rx: rx_arc,
                sub_id: 0,
                pending: 0,
                total: 0,
                start_at: None,
                done_gen: 0,
                handles: HashMap::new(),
            },
            watcher_tx: wtx,
            watcher_rx: wrx_arc,
            watchers: Vec::new(),
            pending_file_events: Vec::new(),
            watcher_debounce_id: 0,
            search_text: String::new(),
            search_debounce_id: 0,
            create_album_input: None,
            rename_album_id: None,
            rename_album_input: String::new(),
            sort_by: SortField::Name,
            sort_asc: true,
            filters: FilterState::default(),
            detail: DetailState::default(),
            show_shortcut_help: false,
            open_menu: None,
            status: initial_status,
            is_syncing: false,
            sync_pending: false,
            welcome: WelcomeState {
                show: show_welcome,
                recent_catalogs,
                selected_recent_catalog: None,
                show_new_catalog_modal: false,
                new_catalog_dir: None,
                new_catalog_name: String::new(),
            },
            album_pending_delete: None,
            folder_pending_remove: None,
            remove_missing_folder: None,
            metadata_import_prompt: None,
            sidebar_scroll_y: 0.0,
            last_click_time: None,
            pending_album_select: None,
            last_synced_path: None,
            remove_from_album_pending: false,
            smart_album_dirty: false,
            context_menu: None,
            hovered_sidebar_entity: None,
            tag_browser: None,
            sidebar_width: SIDEBAR_WIDTH,
            sidebar_resizing: false,
            extensions: Vec::new(),
            settings: SettingsState::default(),
            app_settings: isomfolio_core::app_paths::read_settings(),
            faces: FaceState::default(),
            inference: None,
            inference_manifest: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            compare: CompareState::default(),
            bg_tasks: Vec::new(),
            next_bg_task_id: 0,
            task_panel_open: true,
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

    pub fn scroll_to_index(&self, idx: usize) -> Task<Msg> {
        let cols = self.cols().max(1);
        let step = self.tile_px + TILE_GAP;
        let row = idx / cols;
        let target_y = row as f32 * step + GRID_PADDING;
        let centered = (target_y - self.viewport_height / 2.0 + step / 2.0).max(0.0);
        iced::widget::operation::scroll_to(
            GRID_SCROLL_ID.clone(),
            iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: centered },
        )
    }

    pub fn cols(&self) -> usize {
        let detail_w = if self.detail.show { SIDEBAR_WIDTH } else { 0.0 };
        let avail = (self.viewport_width - 2.0 * GRID_PADDING - detail_w).max(0.0);
        ((avail + TILE_GAP) / (self.tile_px + TILE_GAP)) as usize
    }

    pub fn filter_panel_height(&self) -> f32 {
        if !self.filters.show {
            return 0.0;
        }
        let rows = CRITERIA_ROW_COUNT as f32;
        let spacing = (CRITERIA_ROW_COUNT - 1) as f32 * 6.0;
        let action_row = if self.has_active_filters() {
            CRITERIA_ROW_HEIGHT + 6.0
        } else {
            0.0
        };
        rows * CRITERIA_ROW_HEIGHT + spacing + CRITERIA_PADDING + action_row
    }

    pub fn has_active_filters(&self) -> bool {
        use isomfolio_core::models::FlagFilter;
        !self.filters.tags.is_empty()
            || !self.filters.exts.is_empty()
            || !self.filters.date_from.is_empty()
            || !self.filters.date_to.is_empty()
            || self.filters.flag_filter != FlagFilter::All
            || self.filters.rating_min.is_some()
            || self.filters.hide_rejects
            || self.filters.has_location.is_some()
    }

    pub fn bg_push(&mut self, label: impl Into<String>) -> crate::app::types::BgTaskId {
        let id = self.next_bg_task_id;
        self.next_bg_task_id += 1;
        self.bg_tasks.push(crate::app::types::BgTask {
            id,
            label: label.into(),
            progress: None,
            failed: None,
        });
        self.task_panel_open = true;
        id
    }

    pub fn bg_update(&mut self, id: crate::app::types::BgTaskId, progress: Option<f32>, label: Option<String>) {
        if let Some(t) = self.bg_tasks.iter_mut().find(|t| t.id == id) {
            t.progress = progress;
            if let Some(l) = label { t.label = l; }
        }
    }

    pub fn bg_complete(&mut self, id: crate::app::types::BgTaskId) {
        self.bg_tasks.retain(|t| t.id != id);
    }

    pub fn bg_fail(&mut self, id: crate::app::types::BgTaskId, msg: String) {
        if let Some(t) = self.bg_tasks.iter_mut().find(|t| t.id == id) {
            t.failed = Some(msg);
        }
    }

    pub fn has_any_bg_activity(&self) -> bool {
        !self.bg_tasks.is_empty()
            || self.thumb_ctx.total > 0
            || self.is_syncing
            || self.faces.is_clustering
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
        let effective_flag = if self.filters.hide_rejects && self.filters.flag_filter == FlagFilter::All {
            FlagFilter::NotReject
        } else {
            self.filters.flag_filter
        };
        SearchQuery {
            text: text_opt,
            tags: self.filters.tags.clone(),
            extensions: self.filters.exts.iter().cloned().collect(),
            date_from: parse_date_str(&self.filters.date_from),
            date_to: parse_date_str(&self.filters.date_to),
            sort_by: self.sort_by,
            sort_asc: self.sort_asc,
            flag_filter: effective_flag,
            rating_min: self.filters.rating_min,
            has_location: self.filters.has_location,
            include_orphaned: self.search_text.is_empty() && !self.has_active_filters(),
            ..Default::default()
        }
    }

    pub fn missing_count(&self) -> usize {
        self.files.iter().filter(|f| f.is_orphaned).count()
    }

    pub(crate) fn start_thumbnail_pool(&mut self) {
        if self.thumb_ctx.pool.is_some() {
            return;
        }
        let tx_ready = self.thumb_ctx.tx.clone();
        let tx_failed = self.thumb_ctx.tx.clone();
        let catalog_dir = self.catalog_dir.clone();
        let concurrency = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        self.thumb_ctx.pool = Some(create_worker_pool(
            &catalog_dir,
            concurrency,
            move |fid, path| {
                let _ = tx_ready.try_send(ThumbnailEvent::Ready(fid, path));
            },
            move |fid, _| {
                let _ = tx_failed.try_send(ThumbnailEvent::Failed(fid));
            },
        ));
    }

    pub(crate) fn enqueue_thumbnails(&mut self) {
        let Some(pool) = &self.thumb_ctx.pool else {
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
            self.thumb_ctx.done_gen += 1;
            if self.thumb_ctx.pending == 0 {
                self.thumb_ctx.total = newly_enqueued;
                self.thumb_ctx.start_at = Some(Instant::now());
            } else {
                self.thumb_ctx.total += newly_enqueued;
            }
            self.thumb_ctx.pending += newly_enqueued;
            self.task_panel_open = true;
        }
    }

    pub(crate) fn enqueue_thumbnails_for_ids(&mut self, ids_and_paths: &[(String, String)]) {
        let Some(pool) = &self.thumb_ctx.pool else { return };
        let catalog_dir = self.catalog_dir.clone();
        let mut newly_enqueued = 0usize;
        for (priority, (id, path)) in ids_and_paths.iter().enumerate() {
            if !self.thumbnails.contains_key(id) {
                self.thumbnails.insert(id.clone(), ThumbnailState::Pending);
                let cache = thumbnail_cache_path(&catalog_dir, id);
                if std::path::Path::new(&cache).exists() {
                    self.thumbnails.insert(id.clone(), ThumbnailState::Ready(cache));
                } else {
                    pool.enqueue(id, path, priority as i32);
                    newly_enqueued += 1;
                }
            }
        }
        if newly_enqueued > 0 {
            self.thumb_ctx.done_gen += 1;
            if self.thumb_ctx.pending == 0 {
                self.thumb_ctx.total = newly_enqueued;
                self.thumb_ctx.start_at = Some(Instant::now());
            } else {
                self.thumb_ctx.total += newly_enqueued;
            }
            self.thumb_ctx.pending += newly_enqueued;
            self.task_panel_open = true;
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
        let Some(catalog) = self.catalog.clone() else {
            return Task::done(Msg::FilesLoaded(Vec::new()));
        };
        let item = self.selected_item.clone();
        let query = self.build_search_query();
        let is_smart = self.current_album_is_smart();

        Task::perform(
            async move {
                let cat = catalog.lock_unwrap();
                match item {
                    SidebarItem::AllFiles => cat.search(&query).unwrap_or_default(),
                    SidebarItem::Folder(path) => {
                        let q = SearchQuery {
                            folder_path: Some(path),
                            folder_recursive: true,
                            ..query
                        };
                        cat.search(&q).unwrap_or_default()
                    }
                    SidebarItem::Album(album_id) => {
                        if is_smart {
                            cat.search(&query).unwrap_or_default()
                        } else {
                            cat.search_manual_album(&album_id, &query).unwrap_or_default()
                        }
                    }
                    SidebarItem::FaceCluster(cluster_id) => {
                        cat.get_files_in_face_cluster(&cluster_id).unwrap_or_default()
                    }
                    SidebarItem::Suggestions => {
                        cat.get_files_with_pending_tags().unwrap_or_default()
                    }
                }
            },
            Msg::FilesLoaded,
        )
    }

    pub fn load_pending_counts_task(&self) -> Task<Msg> {
        let Some(catalog) = self.catalog.clone() else { return Task::none() };
        let ids: Vec<String> = self.files.iter().map(|f| f.id.clone()).collect();
        Task::perform(
            async move {
                let cat = catalog.lock_unwrap();
                let counts = cat.get_pending_counts_for_files(&ids).unwrap_or_default();
                let total = cat.get_pending_tag_count().unwrap_or(0);
                (counts, total)
            },
            |(counts, total)| Msg::PendingCountsLoaded { counts, total },
        )
    }

    pub fn refresh_pending_total_task(&self) -> Task<Msg> {
        let Some(catalog) = self.catalog.clone() else { return Task::none() };
        Task::perform(
            async move {
                let cat = catalog.lock_unwrap();
                cat.get_pending_tag_count().unwrap_or(0)
            },
            Msg::PendingTotalLoaded,
        )
    }

    pub fn load_pending_tag_groups_task(&self) -> Task<Msg> {
        let Some(catalog) = self.catalog.clone() else { return Task::none() };
        Task::perform(
            async move {
                let cat = catalog.lock_unwrap();
                cat.get_pending_tags_grouped().unwrap_or_default()
            },
            Msg::PendingTagGroupsLoaded,
        )
    }

    pub(crate) fn load_sidebar_task(&self) -> Task<Msg> {
        let Some(catalog) = self.catalog.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let cat = catalog.lock_unwrap();
                let raw_folders = cat.get_folder_counts().unwrap_or_default();
                let albums = cat.get_all_albums().unwrap_or_default();
                let album_counts = cat.get_all_album_file_counts().unwrap_or_default();
                drop(cat);
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
        let Some(catalog) = self.catalog.clone() else {
            return Task::none();
        };
        let n = self.grid_selected.len();
        if n == 0 {
            return Task::none();
        }
        if n == 1 {
            let file_id = self.grid_selected.iter().next().expect("n == 1 guarantees one element").clone();
            if self.detail.file_id.as_deref() == Some(file_id.as_str()) {
                return Task::none();
            }
            return Task::perform(
                async move {
                    let cat = catalog.lock_unwrap();
                    let tags_with_conf = cat.get_tags_with_confidence(&file_id).unwrap_or_default();
                    let tags: Vec<String> = tags_with_conf.iter().map(|(t, _)| t.clone()).collect();
                    let tag_confidence: std::collections::HashMap<String, f32> = tags_with_conf.iter().filter_map(|(t, c)| Some((t.clone(), (*c)?))).collect();
                    let pending_tags = cat.get_pending_tags(&file_id).unwrap_or_default();
                    let meta_opt = cat.get_metadata(&file_id).ok().flatten();
                    let (rating, label, title, exif_tech) = match meta_opt {
                        Some(m) => (
                            m.xmp.as_ref().and_then(|x| x.core.rating),
                            m.xmp.as_ref().and_then(|x| x.core.label.clone()),
                            m.xmp.as_ref().and_then(|x| x.dublin_core.title.clone()),
                            m.exif_tech,
                        ),
                        None => (None, None, None, None),
                    };
                    (file_id, tags, tag_confidence, pending_tags, rating, label, title, exif_tech)
                },
                |(file_id, tags, tag_confidence, pending_tags, rating, label, title, exif_tech)| Msg::DetailLoaded {
                    file_id,
                    tags,
                    tag_confidence,
                    pending_tags,
                    rating,
                    label,
                    title,
                    exif_tech,
                },
            );
        }
        let file_ids: Vec<String> = self.grid_selected.iter().cloned().collect();
        Task::perform(
            async move {
                let cat = catalog.lock_unwrap();
                let shared_tags = cat.get_shared_tags(&file_ids).unwrap_or_default();
                (file_ids, shared_tags)
            },
            |(file_ids, tags)| Msg::BatchDetailLoaded { file_ids, tags },
        )
    }

    pub(crate) fn load_all_tags_task(&self) -> Task<Msg> {
        let Some(catalog) = self.catalog.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let cat = catalog.lock_unwrap();
                cat.get_all_tags()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(t, _)| t)
                    .collect()
            },
            Msg::AllTagsLoaded,
        )
    }

    pub(crate) fn load_ratings_task(&self) -> Task<Msg> {
        let Some(catalog) = self.catalog.clone() else {
            return Task::none();
        };
        let file_ids: Vec<String> = self.files.iter().map(|f| f.id.clone()).collect();
        if file_ids.is_empty() {
            return Task::done(Msg::RatingsLoaded(HashMap::new()));
        }
        Task::perform(
            async move {
                let cat = catalog.lock_unwrap();
                file_ids
                    .iter()
                    .filter_map(|id| {
                        let meta = cat.get_metadata(id).ok()??;
                        let r = meta.xmp.as_ref()?.core.rating?;
                        (r > 0).then(|| (id.clone(), r))
                    })
                    .collect()
            },
            Msg::RatingsLoaded,
        )
    }

    pub(crate) fn load_tag_browser_task(&self) -> Task<Msg> {
        let Some(catalog) = self.catalog.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let cat = catalog.lock_unwrap();
                cat.get_all_tags().unwrap_or_default()
            },
            Msg::TagBrowserLoaded,
        )
    }

    pub fn tile_index_at(&self, pos: Point) -> Option<usize> {
        let rel_x = pos.x - self.sidebar_width - SIDEBAR_HANDLE_WIDTH - GRID_PADDING;
        let criteria_h = self.filter_panel_height();
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
        let event_sub = event::listen_with(|event, status, _id| {
            let ignored = status == iced::event::Status::Ignored;
            match event {
                Event::Mouse(mouse::Event::CursorMoved { position }) => Some(Msg::MouseMoved(position)),
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if ignored => {
                    Some(Msg::MousePressed)
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if ignored => {
                    Some(Msg::MouseReleased)
                }
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) if ignored => {
                    Some(Msg::MouseRightClicked)
                }
                Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => Some(Msg::ModifiersChanged(m)),
                Event::Keyboard(keyboard::Event::KeyPressed { ref key, modifiers, .. }) => {
                    keybinds::match_event(keybinds::bindings(), key, modifiers, ignored)
                }
                _ => None,
            }
        });

        let thumb_sub = iced::advanced::subscription::from_recipe(ThumbnailRecipe {
            rx: Arc::clone(&self.thumb_ctx.rx),
            id: self.thumb_ctx.sub_id,
        });

        let watcher_sub = iced::advanced::subscription::from_recipe(WatcherRecipe {
            rx: Arc::clone(&self.watcher_rx),
        });

        Subscription::batch([event_sub, thumb_sub, watcher_sub])
    }
}

pub fn sort_field_label(f: SortField) -> &'static str {
    match f {
        SortField::Name => "Name",
        SortField::Date => "Date Shot",
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
