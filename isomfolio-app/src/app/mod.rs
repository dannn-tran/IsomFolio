pub mod keybinds;
mod types;
mod update;

pub use types::*;

use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

use iced::{event, keyboard, mouse, Event, Point, Size, Subscription, Task};

use isomfolio_core::app_paths::db_path;
use isomfolio_core::Catalog;
use isomfolio_core::indexing::thumbnail::{
    create_worker_pool, thumbnail_cache_path, ThumbnailPool,
};
use isomfolio_core::indexing::types::FileEvent;
use isomfolio_core::indexing::watcher::{create_watcher, start_mount_watch, FileWatcher, MountWatch};
use isomfolio_core::models::SearchQuery;
use isomfolio_core::models::{Album, AlbumId, AlbumKind, AssetFile, SortField, ThumbnailState};

/// Platform name for the OS trash, used in user-facing delete copy.
pub fn os_trash_name() -> &'static str {
    if cfg!(target_os = "windows") { "Recycle Bin" } else { "Trash" }
}

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
    /// Zoom factor (1.0 = fit-to-window) and pan offset in screen pixels.
    /// Shared by the trackpad gestures and the zoom buttons.
    pub zoom: f32,
    pub pan: iced::Vector,
    /// Whether the full-demosaic (hi-res) decode has been swapped in for the
    /// current RAW. The fit view shows the fast embedded preview; the full
    /// decode loads on first zoom-in. Reset on navigate.
    pub hires_loaded: bool,
    /// Last-known loupe image area and native image size, reported by the
    /// `LoupeImage` widget on hover — used to compute the exact "1:1" zoom.
    pub viewport: Option<iced::Size>,
    pub native: Option<iced::Size>,
    /// When set, navigating to another photo keeps the current zoom + pan
    /// (focus-checking the same spot across a burst). A *mode*, so `reset_zoom`
    /// (which fires on navigate) leaves it alone.
    pub lock_zoom: bool,
}

pub const LOUPE_ZOOM_MIN: f32 = 1.0;
pub const LOUPE_ZOOM_MAX: f32 = 8.0;

impl LoupeState {
    /// Reset zoom/pan back to fit-to-window (on open and on navigate).
    pub fn reset_zoom(&mut self) {
        self.zoom = 1.0;
        self.pan = iced::Vector::ZERO;
        self.hires_loaded = false;
    }
}

impl Default for LoupeState {
    fn default() -> Self {
        Self {
            idx: 0,
            full_res: None,
            prefetch: HashMap::new(),
            zoom: 1.0,
            pan: iced::Vector::ZERO,
            hires_loaded: false,
            viewport: None,
            native: None,
            lock_zoom: false,
        }
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
    /// Cluster ids selected for a batch name-and-merge (Cmd/Ctrl-click).
    pub selected: HashSet<String>,
    /// Name typed into the batch bar when a multi-selection is active.
    pub batch_name_input: String,
    pub status: Option<String>,
    pub is_clustering: bool,
    /// Embedding progress 0.0–1.0 while clustering; `None` = indeterminate
    /// (engine starting / model download).
    pub progress: Option<f32>,
}

impl Default for FaceState {
    fn default() -> Self {
        Self { clusters: Vec::new(), crop_handles: HashMap::new(), rename_cluster_id: None, rename_input: String::new(), selected: HashSet::new(), batch_name_input: String::new(), status: None, is_clustering: false, progress: None }
    }
}

#[derive(Debug, Clone)]
pub struct AddFolderPrompt {
    pub path: String,
    pub recursive: bool,
    pub subfolder_count: usize,
}

pub struct App {
    pub catalog: Option<Arc<Mutex<Catalog>>>,
    pub catalog_dir: String,

    pub view_mode: ViewMode,
    pub loupe: LoupeState,

    pub folders: Vec<(String, String, usize)>,
    pub folder_tree: Vec<isomfolio_core::folder_tree::FolderNode>,
    pub expanded_folders: HashSet<String>,
    pub library_roots: Vec<isomfolio_core::LibraryRoot>,
    /// Normalised paths of library roots currently unreachable on disk (unplugged
    /// drive). Recomputed on each sidebar load; files under them render offline.
    pub offline_roots: HashSet<String>,
    pub cameras: Vec<String>,
    pub pending_restore: Option<SidebarItem>,
    /// Folder subtree to auto-expand once the sidebar tree reloads (set after a
    /// sync so a freshly-added folder's children are revealed, not collapsed).
    pub expand_under_path: Option<String>,
    pub add_folder_prompt: Option<AddFolderPrompt>,
    pub albums: Vec<Album>,
    pub album_counts: HashMap<String, usize>,
    /// Album that the `B` quick-add key drops the selection into, if set.
    pub target_album: Option<AlbumId>,
    pub selected_item: SidebarItem,

    pub files: Vec<AssetFile>,
    pub file_ratings: HashMap<String, i32>,
    pub file_labels: HashMap<String, String>,
    /// file_id → burst size, for the ⧉ badge (only files in a burst).
    pub file_burst_sizes: HashMap<String, usize>,
    /// When set, a burst shows as one representative tile.
    pub collapse_bursts: bool,
    pub thumbnails: HashMap<String, ThumbnailState>,
    pub grid_selected: HashSet<String>,
    pub tile_px: f32,
    pub anchor_idx: Option<usize>,
    /// Moving end of a range-selection (Shift+Arrow / Shift+click). `anchor_idx`
    /// is the fixed end; the selection spans the two.
    pub select_lead: Option<usize>,
    /// Selection snapshot taken when the anchor was last set (plain/Cmd click).
    /// A Shift range is computed as `base ∪ [anchor..=lead]`, so Shift can both
    /// grow and shrink the range while preserving disjoint Cmd-selected tiles.
    pub selection_base: std::collections::HashSet<String>,
    /// Last focused grid index per sidebar item (token), so returning to a
    /// folder/album restores the grid position instead of jumping to the top.
    pub saved_positions: HashMap<String, usize>,
    /// Grid index to restore after the next `FilesLoaded` (set when switching
    /// to a view that has a remembered position).
    pub pending_restore_idx: Option<usize>,

    pub scroll_y: f32,
    pub viewport_height: f32,
    pub viewport_width: f32,
    /// Window width (logical px), tracked from window resize events. Column count
    /// derives from this analytically (minus the known panel widths) so it stays
    /// correct on resize / sidebar-drag / detail-toggle — unlike the scroll-only
    /// `viewport_width`, which is stale until the next scroll.
    pub window_width: f32,

    pub cursor: Point,
    pub drag: DragContext,
    pub modifiers: keyboard::Modifiers,

    pub thumb_ctx: ThumbnailContext,

    /// Folders the watcher has flagged as changed-on-disk since the last sync.
    /// The watcher never auto-applies — these surface as a badge and the user
    /// applies them by syncing (transparency: see project_image_loading_design).
    pub dirty_folders: HashSet<String>,

    /// Directories found by an in-progress scan (`path_key → path_display`) but
    /// not yet indexed. Session-only — unioned into the folder tree so subfolders
    /// show immediately on a recursive add, then become real once their files are
    /// indexed. Not persisted; cleared on catalog switch.
    pub discovered_folders: std::collections::HashMap<String, String>,

    pub watcher_tx: mpsc::SyncSender<FileEvent>,
    pub watcher_rx: Arc<std::sync::Mutex<Option<mpsc::Receiver<FileEvent>>>>,
    pub watchers: Vec<(String, FileWatcher)>,
    pub pending_file_events: Vec<FileEvent>,
    /// Event-driven removable-drive detection: a mount/unmount under the OS mount
    /// dirs pushes a tick consumed by `MountRecipe` → `RecheckOfflineRoots`. The
    /// watcher handle is kept alive for the app's lifetime; `None` where there's
    /// no mount directory to watch (Windows — the 5 s poll covers it).
    pub mount_rx: Arc<std::sync::Mutex<Option<mpsc::Receiver<()>>>>,
    pub _mount_watcher: Option<MountWatch>,
    pub watcher_debounce_id: u64,

    pub search_text: String,
    pub search_debounce_id: u64,
    pub create_album_input: Option<String>,
    pub rename_album_id: Option<AlbumId>,
    pub rename_album_input: String,

    pub sort_by: SortField,
    pub sort_asc: bool,
    pub grid_layout: GridLayout,
    pub list_col: ListColWidths,
    pub list_resize: Option<ListResize>,

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
    pub sidebar_scroll_y: f32,

    pub last_click_time: Option<Instant>,
    pub pending_album_select: Option<AlbumId>,
    pub last_synced_path: Option<String>,
    pub remove_from_album_pending: bool,
    /// Confirm state for "Delete Rejected Photos" (acts on the current view).
    pub reject_delete_pending: bool,
    /// Count of soft-deleted photos (drives the sidebar "Deleted" entry).
    pub deleted_count: usize,
    /// Recent import batches (newest first) for the sidebar Imports section.
    pub import_batches: Vec<isomfolio_core::models::ImportBatch>,
    /// Whether the Imports section is expanded past the recent-10 cutoff.
    pub show_all_imports: bool,
    /// Sidebar sections the user has collapsed (hidden their row lists).
    pub collapsed_sections: HashSet<crate::app::types::SidebarSection>,
    /// Pending permanent-purge confirmation: the (id, path) pairs to delete from
    /// disk + catalog. `Some` shows the inline confirm; this is the one delete
    /// path that actually touches files on disk.
    pub purge_pending: Option<Vec<(String, String)>>,
    pub smart_album_dirty: bool,
    pub context_menu: Option<ContextMenuState>,
    pub hovered_sidebar_entity: Option<SidebarItem>,
    pub tag_browser: Option<TagBrowserState>,

    pub sidebar_width: f32,
    pub sidebar_resizing: bool,

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
    /// Recently-finished tasks, shown with a ✓ until they expire (`COMPLETED_TTL`).
    pub completed_tasks: Vec<crate::app::types::CompletedTask>,
    pub next_bg_task_id: crate::app::types::BgTaskId,
    pub task_panel_open: bool,
    pub fullscreen: bool,
}

/// How long a finished task lingers in the panel before expiring.
pub const COMPLETED_TTL: std::time::Duration = std::time::Duration::from_secs(4);

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

/// Bridges mount/unmount ticks from the mount watcher into the update loop as
/// `RecheckOfflineRoots`. Mirrors `WatcherRecipe`.
struct MountRecipe {
    rx: Arc<std::sync::Mutex<Option<mpsc::Receiver<()>>>>,
}

impl iced::advanced::subscription::Recipe for MountRecipe {
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
            None::<mpsc::Receiver<()>>,
            move |rx| {
                let rx_arc = rx_arc.clone();
                async move {
                    let rx = match rx {
                        Some(r) => r,
                        None => rx_arc.lock().ok()?.take()?,
                    };
                    let rx = tokio::task::spawn_blocking(move || rx.recv().ok().map(|_| rx))
                        .await
                        .ok()
                        .flatten()?;
                    Some((Msg::RecheckOfflineRoots, Some(rx)))
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

        // Event-driven mount detection: watch the OS mount dirs; each mount change
        // invalidates the volume snapshot and ticks the offline recheck.
        let (mtx, mrx) = mpsc::sync_channel::<()>(8);
        let mount_rx = Arc::new(std::sync::Mutex::new(Some(mrx)));
        let mount_watcher = {
            let mtx = mtx.clone();
            start_mount_watch(move || {
                isomfolio_core::volume::invalidate_cache();
                let _ = mtx.try_send(());
            })
        };

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
            folder_tree: Vec::new(),
            expanded_folders: HashSet::new(),
            library_roots: Vec::new(),
            offline_roots: HashSet::new(),
            cameras: Vec::new(),
            pending_restore: None,
            expand_under_path: None,
            dirty_folders: HashSet::new(),
            discovered_folders: std::collections::HashMap::new(),
            add_folder_prompt: None,
            albums: Vec::new(),
            album_counts: HashMap::new(),
            target_album: None,
            selected_item: SidebarItem::AllFiles,
            files: Vec::new(),
            file_ratings: HashMap::new(),
            file_labels: HashMap::new(),
            file_burst_sizes: HashMap::new(),
            collapse_bursts: false,
            thumbnails: HashMap::new(),
            grid_selected: HashSet::new(),
            tile_px: 180.0,
            anchor_idx: None,
            select_lead: None,
            selection_base: HashSet::new(),
            saved_positions: HashMap::new(),
            pending_restore_idx: None,
            scroll_y: 0.0,
            viewport_height: 600.0,
            viewport_width: 1060.0,
            window_width: 1300.0,
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
            },
            watcher_tx: wtx,
            watcher_rx: wrx_arc,
            mount_rx,
            _mount_watcher: mount_watcher,
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
            grid_layout: GridLayout::Grid,
            list_col: ListColWidths::default(),
            list_resize: None,
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
            sidebar_scroll_y: 0.0,
            last_click_time: None,
            pending_album_select: None,
            last_synced_path: None,
            remove_from_album_pending: false,
            reject_delete_pending: false,
            deleted_count: 0,
            import_batches: Vec::new(),
            show_all_imports: false,
            // Filters starts collapsed: the sidebar opens on navigation (All
            // Photos / Folders / Albums), not a wall of criteria. The pinned
            // footer header + `●` marker keep it one click away.
            collapsed_sections: {
                let mut s = HashSet::new();
                s.insert(crate::app::types::SidebarSection::Filters);
                s
            },
            purge_pending: None,
            smart_album_dirty: false,
            context_menu: None,
            hovered_sidebar_entity: None,
            tag_browser: None,
            sidebar_width: SIDEBAR_WIDTH,
            sidebar_resizing: false,
            settings: SettingsState::default(),
            app_settings: isomfolio_core::app_paths::read_settings(),
            faces: FaceState::default(),
            inference: None,
            inference_manifest: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            compare: CompareState::default(),
            bg_tasks: Vec::new(),
            completed_tasks: Vec::new(),
            next_bg_task_id: 0,
            task_panel_open: true,
            fullscreen: false,
        };

        (app, task)
    }

    pub(crate) fn resize_to_main() -> Task<Msg> {
        let new_size = Size::new(1280.0, 800.0);
        iced::window::oldest().then(move |opt_id| {
            let Some(id) = opt_id else {
                return Task::none();
            };
            // Grow from the centre, not the top-left corner: shift the origin up
            // and left by half the size increase so the window's centre is fixed.
            iced::window::size(id).then(move |old_size| {
                iced::window::position(id).then(move |old_pos| {
                    let resize = iced::window::resize(id, new_size);
                    match old_pos {
                        Some(p) => {
                            let dx = (new_size.width - old_size.width) / 2.0;
                            let dy = (new_size.height - old_size.height) / 2.0;
                            let centred = iced::Point::new(p.x - dx, p.y - dy);
                            Task::batch([resize, iced::window::move_to(id, centred)])
                        }
                        None => resize,
                    }
                })
            })
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
        let step = self.row_step();
        let row = idx / cols;
        let target_y = row as f32 * step + GRID_PADDING;
        let centered = (target_y - self.viewport_height / 2.0 + step / 2.0).max(0.0);
        iced::widget::operation::scroll_to(
            GRID_SCROLL_ID.clone(),
            iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: centered },
        )
    }

    /// Vertical distance between consecutive rows in the content area.
    /// One column means one file per row (List); the grid packs `cols` per row.
    pub fn row_step(&self) -> f32 {
        match self.grid_layout {
            GridLayout::List => LIST_ROW_HEIGHT,
            GridLayout::Grid => self.tile_px + TILE_GAP,
        }
    }

    pub fn cols(&self) -> usize {
        if matches!(self.grid_layout, GridLayout::List) {
            return 1;
        }
        // Derive the grid's usable width from the window minus the panels flanking
        // it — sidebar, its resize handle, and (once) the detail panel when shown —
        // then the grid's own padding and scrollbar. Computing it here (rather than
        // reading the scroll-sourced `viewport_width`) keeps the column count exact
        // the instant any of those widths change, without waiting for a scroll.
        let detail_w = if self.detail.show { SIDEBAR_WIDTH } else { 0.0 };
        let grid_w = self.window_width - self.sidebar_width - SIDEBAR_HANDLE_WIDTH - detail_w;
        let avail = (grid_w - 2.0 * GRID_PADDING - GRID_SCROLLBAR_WIDTH).max(0.0);
        (((avail + TILE_GAP) / (self.tile_px + TILE_GAP)) as usize).max(1)
    }

    pub fn has_active_filters(&self) -> bool {
        !self.filters.tags.is_empty()
            || !self.filters.exclude_tags.is_empty()
            || !self.filters.exts.is_empty()
            || !self.filters.date_from.is_empty()
            || !self.filters.date_to.is_empty()
            || self.filters.flags.is_active()
            || self.filters.rating.is_active()
            || self.filters.has_location.is_some()
            || self.filters.person.is_some()
            || self.filters.added_within_days.is_some()
            || self.filters.camera.is_some()
            || self.filters.color.is_some()
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

    pub fn bg_complete(&mut self, id: crate::app::types::BgTaskId) {
        if let Some(t) = self.bg_tasks.iter().find(|t| t.id == id) {
            let label = t.label.clone();
            self.bg_mark_done(label, String::new());
        }
        self.bg_tasks.retain(|t| t.id != id);
    }

    /// Record a finished task so it lingers with a ✓ instead of vanishing. Newest
    /// first, capped so a long session doesn't accumulate stale toasts.
    pub fn bg_mark_done(&mut self, title: impl Into<String>, detail: impl Into<String>) {
        self.completed_tasks.insert(
            0,
            crate::app::types::CompletedTask {
                title: title.into(),
                detail: detail.into(),
                at: std::time::Instant::now(),
            },
        );
        self.completed_tasks.truncate(5);
        self.task_panel_open = true;
    }

    pub fn bg_fail(&mut self, id: crate::app::types::BgTaskId, msg: String) {
        if let Some(t) = self.bg_tasks.iter_mut().find(|t| t.id == id) {
            t.failed = Some(msg);
        }
    }

    pub fn has_any_bg_activity(&self) -> bool {
        !self.bg_tasks.is_empty()
            || !self.completed_tasks.is_empty()
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

    /// Persist the current catalog + selected view so it can be restored next launch.
    pub fn save_session(&self) {
        if self.catalog_dir.is_empty() {
            return;
        }
        isomfolio_core::app_paths::save_session(&isomfolio_core::app_paths::Session {
            catalog_path: self.catalog_dir.clone(),
            folders: Vec::new(),
            last_selected: Some(self.selected_item.to_token()),
        });
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
        SearchQuery {
            text: text_opt,
            tags: self.filters.tags.clone(),
            tag_match: self.filters.tag_match,
            exclude_tags: self.filters.exclude_tags.clone(),
            extensions: self.filters.exts.iter().cloned().collect(),
            date_from: parse_date_str(&self.filters.date_from),
            date_to: parse_date_str(&self.filters.date_to),
            sort_by: self.sort_by,
            sort_asc: self.sort_asc,
            flags: self.filters.flags,
            rating: self.filters.rating,
            has_location: self.filters.has_location,
            person_cluster: self.filters.person.clone(),
            camera_model: self.filters.camera.clone(),
            color_label: self.filters.color.clone(),
            added_within_days: self.filters.added_within_days,
            include_orphaned: self.search_text.is_empty() && !self.has_active_filters(),
            collapse_bursts: self.collapse_bursts,
            ..Default::default()
        }
    }

    pub fn missing_count(&self) -> usize {
        self.files.iter().filter(|f| f.is_orphaned).count()
    }

    /// Whether `path` sits under a library root that's currently offline
    /// (unplugged drive). Cheap: `offline_roots` is empty in the common case, so
    /// the scan short-circuits.
    pub fn is_offline_path(&self, path: &str) -> bool {
        self.offline_roots.iter().any(|r| {
            path == r || path.starts_with(&format!("{r}{}", std::path::MAIN_SEPARATOR))
        })
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
            if self.thumbnails.contains_key(&file.id) {
                continue;
            }
            let cache = thumbnail_cache_path(&catalog_dir, &file.id);
            if std::path::Path::new(&cache).exists() {
                // On disk already (e.g. catalog reopen) — mark Ready; the renderer
                // decodes the JPEG by path on demand, no in-app load.
                self.thumbnails.insert(file.id.clone(), ThumbnailState::Ready(cache));
            } else {
                self.thumbnails.insert(file.id.clone(), ThumbnailState::Pending);
                pool.enqueue(&file.id, &file.path, priority as i32);
                newly_enqueued += 1;
            }
        }
        // Pull the just-loaded view to the front of the queue (in display order),
        // even for files enqueued earlier under a different view — switching to a
        // folder should generate its thumbnails ahead of any remaining backlog.
        let view_ids: Vec<String> = self.files.iter().map(|f| f.id.clone()).collect();
        pool.prioritize(&view_ids);
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

    /// Force-regenerate cached thumbnails for the given paths (content changed
    /// on disk but the path — and thus the file id — is unchanged). Busts the
    /// stale cache entry and re-enqueues. User metadata is untouched.
    pub(crate) fn refresh_thumbnails(&mut self, paths: &[String]) {
        if self.thumb_ctx.pool.is_none() {
            return;
        }
        let catalog_dir = self.catalog_dir.clone();
        let mut enqueued = 0usize;
        for path in paths {
            let id = isomfolio_core::file_index::compute_file_id_for_path(path);
            let cache = thumbnail_cache_path(&catalog_dir, &id);
            let _ = std::fs::remove_file(&cache);
            self.thumbnails.insert(id.clone(), ThumbnailState::Pending);
            if let Some(pool) = &self.thumb_ctx.pool {
                pool.enqueue(&id, path, 0);
            }
            enqueued += 1;
        }
        if enqueued > 0 {
            self.thumb_ctx.done_gen += 1;
            if self.thumb_ctx.pending == 0 {
                self.thumb_ctx.total = enqueued;
                self.thumb_ctx.start_at = Some(Instant::now());
            } else {
                self.thumb_ctx.total += enqueued;
            }
            self.thumb_ctx.pending += enqueued;
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
                    SidebarItem::Deleted => {
                        let q = SearchQuery {
                            only_deleted: true,
                            sort_by: query.sort_by,
                            sort_asc: query.sort_asc,
                            ..Default::default()
                        };
                        cat.search(&q).unwrap_or_default()
                    }
                    SidebarItem::Import(batch_id) => {
                        let q = SearchQuery { import_batch: Some(batch_id), ..query };
                        cat.search(&q).unwrap_or_default()
                    }
                }
            },
            Msg::FilesLoaded,
        )
    }

    pub(crate) fn load_sidebar_task(&self) -> Task<Msg> {
        let Some(catalog) = self.catalog.clone() else {
            return Task::none();
        };
        // Directories found by an in-progress scan but not yet indexed — held in
        // memory (session-only) so subfolders show the moment a recursive add is
        // acknowledged. Unioned into the tree; never persisted.
        let discovered: Vec<(String, String)> = self
            .discovered_folders
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Task::perform(
            async move {
                let cat = catalog.lock_unwrap();
                // Fetch folder counts + library roots once; the flat folder list,
                // the folder tree, and offline detection all derive from them.
                let raw_folders = cat.get_folder_counts().unwrap_or_default();
                let library_roots = cat.list_library_roots().unwrap_or_default();
                let cameras = cat.distinct_camera_models().unwrap_or_default();
                let albums = cat.get_all_albums().unwrap_or_default();
                let album_counts = cat.get_all_album_file_counts().unwrap_or_default();
                let deleted_count = cat.count_deleted().unwrap_or(0);
                let import_batches = cat.get_import_batches(None).unwrap_or_default();
                drop(cat);
                // Display basename comes from the stored case-preserved path —
                // no disk read. Falls back to the key path when display is unset.
                let folders: Vec<(String, String, usize)> = raw_folders
                    .iter()
                    .map(|(path, display, count)| {
                        let src = if display.is_empty() { path } else { display };
                        let name = std::path::Path::new(src)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(src)
                            .to_string();
                        (path.clone(), name, *count)
                    })
                    .collect();
                // Built from the already-fetched counts + roots — no re-query.
                let folder_tree =
                    isomfolio_core::Catalog::folder_tree_from(raw_folders, &library_roots, &discovered);
                // A library root whose path isn't a directory right now is offline
                // (unplugged drive). Cheap — roots are few; the key path resolves
                // on case-insensitive filesystems.
                let offline_roots: HashSet<String> = library_roots
                    .iter()
                    .filter(|r| !std::path::Path::new(&r.path).is_dir())
                    .map(|r| r.path.clone())
                    .collect();
                (folders, folder_tree, library_roots, offline_roots, cameras, albums, album_counts, deleted_count, import_batches)
            },
            |(folders, folder_tree, library_roots, offline_roots, cameras, albums, album_counts, deleted_count, import_batches)| {
                Msg::SidebarLoaded {
                    folders,
                    folder_tree,
                    library_roots,
                    offline_roots,
                    cameras,
                    albums,
                    album_counts,
                    deleted_count,
                    import_batches,
                }
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
                    let tags = cat.get_tags_for_file(&file_id).unwrap_or_default();
                    let meta_opt = cat.get_metadata(&file_id).ok().flatten();
                    let (rating, label, title, description, creator, rights, exif_tech) = match meta_opt {
                        Some(m) => {
                            let dc = m.xmp.as_ref().map(|x| &x.dublin_core);
                            (
                                m.xmp.as_ref().and_then(|x| x.core.rating),
                                m.xmp.as_ref().and_then(|x| x.core.label.clone()),
                                dc.and_then(|d| d.title.clone()),
                                dc.and_then(|d| d.description.clone()),
                                dc.map(|d| d.creator.join("; ")).filter(|s| !s.is_empty()),
                                dc.and_then(|d| d.rights.clone()),
                                m.exif_tech,
                            )
                        }
                        None => (None, None, None, None, None, None, None),
                    };
                    (file_id, tags, rating, label, title, description, creator, rights, exif_tech)
                },
                |(file_id, tags, rating, label, title, description, creator, rights, exif_tech)| Msg::DetailLoaded {
                    file_id,
                    tags,
                    rating,
                    label,
                    title,
                    description,
                    creator,
                    rights,
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

    /// Load the per-file side data the grid shows alongside each tile — ratings,
    /// colour labels, burst sizes — in one task (one catalog lock), each via a
    /// batched query rather than a per-file round-trip.
    pub(crate) fn load_file_side_data_task(&self) -> Task<Msg> {
        let Some(catalog) = self.catalog.clone() else {
            return Task::none();
        };
        let file_ids: Vec<String> = self.files.iter().map(|f| f.id.clone()).collect();
        if file_ids.is_empty() {
            return Task::done(Msg::FileSideDataLoaded {
                ratings: HashMap::new(),
                labels: HashMap::new(),
                bursts: HashMap::new(),
            });
        }
        Task::perform(
            async move {
                let cat = catalog.lock_unwrap();
                let ratings = cat.get_ratings_for(&file_ids).unwrap_or_default();
                let labels = cat.get_file_labels(&file_ids).unwrap_or_default();
                let bursts = cat.get_burst_sizes_for(&file_ids).unwrap_or_default();
                (ratings, labels, bursts)
            },
            |(ratings, labels, bursts)| Msg::FileSideDataLoaded { ratings, labels, bursts },
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

    /// True when `pos` falls in the List column-header strip (sort buttons and
    /// resize handles). Clicks there must not touch the grid selection.
    pub fn in_list_header_band(&self, pos: Point) -> bool {
        if !matches!(self.grid_layout, GridLayout::List) {
            return false;
        }
        let top = TOOLBAR_HEIGHT;
        pos.x > self.sidebar_width + SIDEBAR_HANDLE_WIDTH
            && pos.y >= top
            && pos.y < top + LIST_HEADER_HEIGHT
    }

    pub fn tile_index_at(&self, pos: Point) -> Option<usize> {
        let rel_x = pos.x - self.sidebar_width - SIDEBAR_HANDLE_WIDTH - GRID_PADDING;
        let list_header = match self.grid_layout {
            GridLayout::List => LIST_HEADER_HEIGHT,
            GridLayout::Grid => 0.0,
        };
        let rel_y = pos.y + self.scroll_y
            - TOOLBAR_HEIGHT
            - list_header
            - GRID_PADDING;
        if rel_x < 0.0 || rel_y < 0.0 {
            return None;
        }

        if matches!(self.grid_layout, GridLayout::List) {
            // One file per row spanning the full content width — the whole row
            // is the hit target (no inter-tile gaps to reject).
            let idx = (rel_y / LIST_ROW_HEIGHT) as usize;
            return (idx < self.files.len()).then_some(idx);
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
                Event::Window(iced::window::Event::Resized(size)) => {
                    Some(Msg::WindowResized(size.width))
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

        let mount_sub = iced::advanced::subscription::from_recipe(MountRecipe {
            rx: Arc::clone(&self.mount_rx),
        });

        let mut subs = vec![event_sub, thumb_sub, watcher_sub, mount_sub];
        // Tick only while completed toasts are lingering, so they auto-expire.
        if !self.completed_tasks.is_empty() {
            subs.push(
                iced::time::every(std::time::Duration::from_secs(1))
                    .map(|_| Msg::PruneCompletedTasks),
            );
        }
        // Poll removable-drive reachability so an unplug/remount is reflected
        // without the user having to trigger a reload. Coarse (5 s) and only when
        // there are roots to watch; the check itself runs off-thread.
        if !self.library_roots.is_empty() {
            subs.push(
                iced::time::every(std::time::Duration::from_secs(5))
                    .map(|_| Msg::RecheckOfflineRoots),
            );
        }
        Subscription::batch(subs)
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

/// Compute `(from, to)` `YYYY-MM-DD` strings for a date preset, relative to today.
pub fn date_preset_range(preset: crate::app::DatePreset) -> (String, String) {
    use crate::app::DatePreset;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let today = unix_to_date_str(now);
    let from = match preset {
        DatePreset::Last7 => unix_to_date_str(now - 7 * 86400),
        DatePreset::Last30 => unix_to_date_str(now - 30 * 86400),
        DatePreset::ThisMonth => format!("{}-01", &today[..7]),
        DatePreset::ThisYear => format!("{}-01-01", &today[..4]),
    };
    (from, today)
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

#[cfg(test)]
mod layout_tests {
    use super::*;

    fn app() -> App {
        App::new(None).0
    }

    #[test]
    fn list_layout_is_single_column_with_list_row_step() {
        let mut a = app();
        a.grid_layout = GridLayout::List;
        // Width/tile size are irrelevant in List — always one file per row.
        a.window_width = 2000.0;
        a.tile_px = 180.0;
        assert_eq!(a.cols(), 1);
        assert_eq!(a.row_step(), LIST_ROW_HEIGHT);
    }

    #[test]
    fn grid_layout_packs_columns_from_width_and_tile_size() {
        let mut a = app();
        a.grid_layout = GridLayout::Grid;
        a.window_width = 1000.0;
        a.sidebar_width = SIDEBAR_WIDTH;
        a.tile_px = 180.0;
        a.detail.show = false;
        assert!(a.cols() >= 2);
        assert_eq!(a.row_step(), a.tile_px + TILE_GAP);
    }

    #[test]
    fn detail_panel_costs_exactly_one_panel_width_not_two() {
        // Regression: cols() once subtracted the detail-panel width on top of the
        // already-narrower grid measurement, charging the panel twice and dropping
        // an extra column when it opened. Invariant: opening the panel at window
        // width W must equal closing it at width W − one panel width.
        let mut a = app();
        a.grid_layout = GridLayout::Grid;
        a.sidebar_width = SIDEBAR_WIDTH;
        a.tile_px = 180.0;

        a.window_width = 1400.0;
        a.detail.show = false;
        let wide = a.cols();
        a.detail.show = true;
        let narrow = a.cols();
        assert!(wide >= narrow, "opening detail should never add columns");

        a.detail.show = false;
        a.window_width = 1400.0 - SIDEBAR_WIDTH;
        assert_eq!(
            narrow,
            a.cols(),
            "detail panel must cost exactly one panel width, not two",
        );
    }

    #[test]
    fn list_col_set_clamps_to_bounds() {
        let mut w = ListColWidths::default();
        w.set(ListCol::Date, 5.0);
        assert_eq!(w.date, LIST_COL_MIN);
        w.set(ListCol::Date, 10_000.0);
        assert_eq!(w.date, LIST_COL_MAX);
        w.set(ListCol::Name, 200.0);
        assert_eq!(w.get(ListCol::Name), 200.0);
    }
}
