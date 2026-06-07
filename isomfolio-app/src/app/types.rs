use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use std::sync::LazyLock;
use iced::{keyboard, widget, Point};

pub static GRID_SCROLL_ID: LazyLock<widget::Id> = LazyLock::new(|| widget::Id::unique());
pub static SIDEBAR_SCROLL_ID: LazyLock<widget::Id> = LazyLock::new(|| widget::Id::unique());
use isomfolio_core::extension::ExtensionManifest;
use isomfolio_core::folder_tree::FolderNode;
use isomfolio_core::models::{Album, AlbumId, AssetFile, Flag, Shelf, ShelfId, SortField};
use isomfolio_core::LibraryRoot;

#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    Folder(String),
    ManualAlbum(AlbumId),
    SmartAlbum(AlbumId),
    GridTiles,
    FaceCluster(String),
    Shelf(ShelfId),
    /// The Albums section's "+" affordance: a small menu to create a new album
    /// or a new shelf. Replaces two near-identical add glyphs in the header.
    AlbumsAdd,
}

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub position: Point,
    pub target: ContextMenuTarget,
    pub submenu_open: bool,
}

pub const SIDEBAR_WIDTH: f32 = 220.0;
pub const SIDEBAR_HANDLE_WIDTH: f32 = 5.0;
pub const GRID_PADDING: f32 = 12.0;
pub const TILE_GAP: f32 = 8.0;
/// Width the grid's vertical scrollbar reserves — must match the `Scrollbar`
/// width set on the grid scrollable in `content.rs`, so column math leaves room
/// for it instead of letting the last tile wrap.
pub const GRID_SCROLLBAR_WIDTH: f32 = 6.0;
pub const ALBUM_ITEM_HEIGHT: f32 = 32.0;
pub const FOLDER_ITEM_HEIGHT: f32 = 28.0;
pub const DRAG_THRESHOLD: f32 = 6.0;
pub const TILE_SIZE_STEP: f32 = 40.0;
pub const TILE_SIZE_MIN: f32 = 80.0;
pub const TILE_SIZE_MAX: f32 = 400.0;
pub const SIDEBAR_WIDTH_MIN: f32 = 140.0;
pub const SIDEBAR_WIDTH_MAX: f32 = 400.0;
pub const BUFFER_ROWS: usize = 2;
/// Height of the minimal content-area toolbar (sort · view mode · grid size).
pub const TOOLBAR_HEIGHT: f32 = 40.0;
/// One row in the compact List layout (thumbnail + columns).
pub const LIST_ROW_HEIGHT: f32 = 32.0;
/// Clickable column-header strip shown above the grid in List layout only.
/// Counted in `tile_index_at` so list-row hit-testing stays exact.
pub const LIST_HEADER_HEIGHT: f32 = 24.0;
/// Clamp range for a user-dragged List column width.
pub const LIST_COL_MIN: f32 = 44.0;
pub const LIST_COL_MAX: f32 = 360.0;

/// The user-resizable columns in the List layout (thumbnail/flag/colour are
/// fixed glyph columns and not resizable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListCol {
    Name,
    Stars,
    Date,
    Size,
    Type,
}

/// Current widths (px) of the resizable List columns. Runtime state, in-memory.
#[derive(Debug, Clone, Copy)]
pub struct ListColWidths {
    pub name: f32,
    pub stars: f32,
    pub date: f32,
    pub size: f32,
    pub type_: f32,
}

impl Default for ListColWidths {
    fn default() -> Self {
        Self { name: 260.0, stars: 76.0, date: 116.0, size: 84.0, type_: 60.0 }
    }
}

impl ListColWidths {
    pub fn get(&self, col: ListCol) -> f32 {
        match col {
            ListCol::Name => self.name,
            ListCol::Stars => self.stars,
            ListCol::Date => self.date,
            ListCol::Size => self.size,
            ListCol::Type => self.type_,
        }
    }

    pub fn set(&mut self, col: ListCol, w: f32) {
        let w = w.clamp(LIST_COL_MIN, LIST_COL_MAX);
        match col {
            ListCol::Name => self.name = w,
            ListCol::Stars => self.stars = w,
            ListCol::Date => self.date = w,
            ListCol::Size => self.size = w,
            ListCol::Type => self.type_ = w,
        }
    }
}

/// In-flight column drag: which column, plus the cursor x and column width at
/// drag start so width tracks the cursor delta.
#[derive(Debug, Clone, Copy)]
pub struct ListResize {
    pub col: ListCol,
    pub start_x: f32,
    pub start_w: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    Browse,
    Preview,
    Loupe,
    People,
    Compare,
    Settings,
}

/// How the Browse content area lays out files: a thumbnail grid or a compact
/// columnar list (filename · flag · rating · date · size · type). A sub-mode of
/// Browse — filters, cull strip, and detail panel stay live in both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridLayout {
    Grid,
    List,
}

/// Collapsible sidebar sections that hold a list of rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarSection {
    Filters,
    Folders,
    Albums,
    Imports,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SidebarItem {
    AllFiles,
    Folder(String),
    Album(AlbumId),
    /// Virtual view of soft-deleted photos.
    Deleted,
    /// A discrete import batch (a sync that added files), by batch id.
    Import(i64),
}

impl SidebarItem {
    /// Serialize to a stable token for persisting the last-selected view.
    pub fn to_token(&self) -> String {
        match self {
            SidebarItem::AllFiles => "all".to_string(),
            SidebarItem::Folder(p) => format!("folder:{p}"),
            SidebarItem::Album(id) => format!("album:{id}"),
            SidebarItem::Deleted => "deleted".to_string(),
            SidebarItem::Import(id) => format!("import:{id}"),
        }
    }

    pub fn from_token(s: &str) -> Option<Self> {
        match s {
            "all" => Some(SidebarItem::AllFiles),
            "deleted" => Some(SidebarItem::Deleted),
            _ => {
                let (kind, rest) = s.split_once(':')?;
                match kind {
                    "folder" => Some(SidebarItem::Folder(rest.to_string())),
                    "album" => Some(SidebarItem::Album(rest.to_string())),
                    "import" => rest.parse().ok().map(SidebarItem::Import),
                    _ => None,
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct DragState {
    pub origin_idx: usize,
    pub start: Point,
    pub cursor: Point,
    pub active: bool,
}

/// An album being dragged from the sidebar toward a shelf. `active` flips once
/// the cursor passes the drag threshold (so a plain click still navigates).
#[derive(Debug, Clone)]
pub struct AlbumDragState {
    pub album_id: AlbumId,
    pub start: Point,
    pub cursor: Point,
    pub active: bool,
}

#[derive(Debug)]
pub enum ThumbnailEvent {
    Ready(String, String),
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum Msg {
    CatalogReady,

    SidebarItemClicked(SidebarItem),

    FilesLoaded(Vec<AssetFile>),
    SidebarLoaded {
        folders: Vec<(String, String, usize)>,
        folder_tree: Vec<FolderNode>,
        library_roots: Vec<LibraryRoot>,
        /// Normalised paths of library roots currently unreachable on disk
        /// (e.g. an unplugged removable drive). Files under them render as offline.
        offline_roots: std::collections::HashSet<String>,
        cameras: Vec<String>,
        albums: Vec<Album>,
        shelves: Vec<Shelf>,
        album_counts: HashMap<String, usize>,
        deleted_count: usize,
        import_batches: Vec<isomfolio_core::models::ImportBatch>,
    },
    ImportBatchesLoaded(Vec<isomfolio_core::models::ImportBatch>),
    ToggleShowAllImports,
    ToggleSidebarSection(SidebarSection),
    /// Periodic tick that expires lingering completed-task entries.
    PruneCompletedTasks,
    /// Periodic tick (while library roots exist) to detect a removable drive
    /// going offline / coming back, off-thread.
    RecheckOfflineRoots,
    /// Result of the off-thread reachability check: the roots currently offline.
    OfflineRootsChecked(std::collections::HashSet<String>),

    TileSizeUp,
    TileSizeDown,
    /// Absolute tile size from the toolbar slider (clamped to the size bounds).
    SetTileSize(f32),
    /// Window resized — carries the new logical width, used to recompute columns.
    WindowResized(f32),

    MouseMoved(Point),
    MousePressed,
    MouseReleased,
    MouseRightClicked,
    ModifiersChanged(keyboard::Modifiers),
    EscapePressed,
    Navigate {
        dx: i32,
        dy: i32,
    },
    /// Shift+Arrow: extend the grid selection toward the new position.
    NavigateExtend {
        dx: i32,
        dy: i32,
    },
    /// Delete/Backspace: remove the selected photos from the current manual
    /// album (non-destructive — files stay on disk and in the catalog).
    DeleteKeyPressed,
    OpenLoupe,
    /// Loupe zoom/pan changed via trackpad/scroll or pan-drag (new scale + offset).
    LoupeZoomChanged { scale: f32, pan: iced::Vector },
    /// Loupe zoom button: multiply the current zoom by `factor` (centred).
    LoupeZoomBy(f32),
    /// Reset loupe zoom/pan to fit-to-window.
    LoupeZoomReset,
    /// Zoom the loupe to 1:1 (actual pixels), computed from reported geometry.
    LoupeZoomActual,
    /// Reported by the loupe image widget: (viewport size, native image size).
    LoupeGeometry { viewport: iced::Size, native: iced::Size },
    /// Toggle OS fullscreen for the main window.
    ToggleFullscreen,
    /// Toggle whether loupe zoom/pan is kept when navigating between photos.
    ToggleLoupeZoomLock,
    /// Jump the loupe directly to a photo (filmstrip click).
    LoupeJumpTo(usize),

    Scrolled {
        y: f32,
        height: f32,
        width: f32,
    },

    DroppedToAlbum(AlbumId, Vec<String>),
    DropCompleted,

    SyncPickFolder,
    /// Open the add-folder picker anchored at an existing folder (context-menu
    /// "Add folder", Capture One style).
    SyncPickFolderAt(String),
    SyncStart { path: String, recursive: bool },
    AddFolderPromptToggleRecursive,
    AddFolderConfirm,
    AddFolderCancel,
    ToggleFolderExpanded(String),
    SyncComplete { count: usize, new_file_ids: Vec<String> },

    StartCreateAlbum,
    CreateAlbumInputChanged(String),
    ConfirmCreateAlbum,
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
    ConfirmRemoveFromAlbum,
    CancelRemoveFromAlbum,

    SortDirToggle,
    SetSortField(SortField),
    SetGridLayout(GridLayout),
    /// Begin dragging a List column's right-edge resize handle.
    ListColResizeStart(ListCol),

    SearchChanged(String),

    ToggleFilterPanel,
    FilterTagInputChanged(String),
    AddFilterTag,
    RemoveFilterTag(String),
    /// Toggle a tag chip between include and exclude (NOT).
    ToggleFilterTagNegate(String),
    /// Set how include tags combine (All = AND, Any = OR).
    SetTagMatch(isomfolio_core::models::TagMatch),
    FilterDateFromChanged(String),
    FilterDateToChanged(String),
    SetDatePreset(DatePreset),
    ToggleFilterFileType(String),
    ClearFilters,

    SaveAsSmartAlbum,
    SmartAlbumNameChanged(String),
    ConfirmSmartAlbum,
    UpdateSmartAlbum,

    ToggleDetail,
    DetailLoaded {
        file_id: String,
        tags: Vec<String>,
        rating: Option<i32>,
        label: Option<String>,
        title: Option<String>,
        description: Option<String>,
        creator: Option<String>,
        rights: Option<String>,
        exif_tech: Option<isomfolio_core::models::ExifTechMeta>,
    },
    DetailFieldChanged(DetailField, String),
    SaveDetailField(DetailField),
    DetailTagInputChanged(String),
    AddDetailTag,
    RemoveDetailTag(String),
    SetDetailRating(i32),
    AllTagsLoaded(Vec<String>),
    AddDetailTagDirect(String),
    BatchDetailLoaded { file_ids: Vec<String>, tags: Vec<String> },
    BatchTagsChanged,
    RepeatLastTag,
    ToggleShortcutHelp,
    OpenMenuDropdown(String),
    HoverMenuTab(String),
    CloseMenuDropdown,
    TogglePreview,

    OpenTagBrowser,
    CloseTagBrowser,
    TagBrowserLoaded(Vec<(String, usize)>),
    TagBrowserFilterChanged(String),
    TagBrowserRenameStart(String),
    TagBrowserRenameChanged(String),
    TagBrowserRenameConfirm,
    TagBrowserRenameCancel,
    TagBrowserDeleteArm(String),
    TagBrowserDeleteConfirm,
    TagBrowserDeleteCancel,
    TagBrowserTagRenamed,
    TagBrowserTagDeleted,

    Reload,
    DbError(String),
    TagsSavedResult(Vec<String>, Option<String>),
    SearchDebounceTimer { id: u64, text: String },
    ClearThumbnailProgress(u64),
    SidebarScrolled(f32),

    PickOpenCatalog,
    OpenCatalogPicked(std::path::PathBuf),
    SelectRecentCatalog(String),
    OpenSelectedRecentCatalog,
    ShowNewCatalogModal,
    HideNewCatalogModal,
    PickNewCatalogDir,
    NewCatalogDirPicked(std::path::PathBuf),
    NewCatalogNameChanged(String),
    ConfirmNewCatalog,
    OpenCatalog(String),

    RequestDeleteAlbum(AlbumId),
    CancelDeleteAlbum,
    RequestRemoveFolder(String),
    CancelRemoveFolder,

    SyncDialogDone(Option<String>),
    SetFlag(Flag),
    SetRating(Option<i32>),
    /// Set (or, when re-applying the same colour, clear) the colour label on the
    /// current selection / loupe photo.
    SetColorLabel(Option<String>),
    SetColorFilter(Option<String>),
    /// Per-file grid side data (ratings, colour labels, burst sizes), loaded
    /// together after a file-list load.
    FileSideDataLoaded {
        ratings: HashMap<String, i32>,
        labels: HashMap<String, String>,
        bursts: HashMap<String, usize>,
    },
    ToggleCollapseBursts,
    FlagsApplied,
    RatingsApplied,
    LabelsApplied,
    ToggleHideRejects,
    ToggleFlagFilter(Flag),
    SetRatingFilter(isomfolio_core::models::RatingFilter),
    SetRatingCmp(RatingCmp),
    SetLocationFilter(Option<bool>),
    SetPersonFilter(Option<String>),
    SetAddedWithinFilter(Option<i64>),
    SetCameraFilter(Option<String>),

    ExtensionsDiscovered(Option<ExtensionManifest>),

    BgTaskDismissed(BgTaskId),
    ToggleTaskPanel,

    OpenSettings,
    SwitchSettingsTab(SettingsTab),
    CloseSettings,
    ToggleAutoFaceCluster,
    ToggleInferenceCustom,
    InferenceUrlChanged(String),
    InferencePortChanged(String),
    FaceEpsChanged(String),
    FaceMinPtsChanged(String),
    ToggleImportXmpTags,
    ToggleImportAppleTags,
    ToggleAutoAdvanceOnFlag,
    ToggleAutoStack,
    StackThresholdChanged(String),
    StackWindowChanged(String),
    SettingsConfigChanged { extension_name: String, key: String, value: String },
    SaveSettings,
    InstallExtensionPickFile,
    ExtensionPackagePicked(Option<String>),
    EngineInstalled(ExtensionManifest),
    ExtensionInstallFailed(String),
    UninstallExtension(String),

    /// Recompute perceptual hashes for any unhashed files (from cached
    /// thumbnails) and re-derive per-folder stacks. Runs in the background.
    RunStacking,
    /// Stacking finished writing `burst_id`s; refresh badges / collapsed view.
    StacksUpdated,

    RunFaceClustering { force_full: bool },
    FaceClusterProgress { files_done: usize, total: usize, percent: u8 },
    InferenceEngineReady {
        client: Result<Arc<crate::inference::InferenceClient>, String>,
        force_full: bool,
    },
    FaceClusteringDone(Vec<isomfolio_core::models::FaceClusterSummary>),
    FaceClustersLoaded(Vec<isomfolio_core::models::FaceClusterSummary>),
    RenameFaceCluster(String),
    RenameFaceClusterInputChanged(String),
    ConfirmRenameFaceCluster,
    MergeFaceClusters(String, String),
    /// A person card was clicked. Plain click navigates; Cmd/Ctrl-click toggles
    /// the card into the batch selection.
    FaceClusterCardClicked(String),
    ClearFaceSelection,
    BatchFaceNameInputChanged(String),
    /// Name the selected clusters as one person and merge them together.
    ConfirmBatchFaceNameMerge,
    FaceCropsReady(Vec<(String, iced::widget::image::Handle)>),
    OpenPeopleView,

    SelectAll,
    DeselectAll,
    OpenFaceClusterMenu(String),
    /// Open the Albums "+" menu (New Album / New Shelf) at the cursor.
    OpenAlbumsAddMenu,
    Undo,
    Redo,
    UndoApplied,
    OpenCompare,
    CompareFullResLoaded { slot: usize, handle: iced::widget::image::Handle },
    NoOp,

    SidebarResizeStart,
    SyncFolder(String),
    SyncSelectedFolder,
    DuplicateAlbum(AlbumId),
    ShowInFinder(Vec<String>),
    AddSelectionToAlbum(AlbumId),
    /// Set (or clear) the album the `B` key quick-adds the selection to.
    SetTargetAlbum(AlbumId),
    /// Add the current selection to the target album (`B`).
    AddSelectionToTargetAlbum,
    HoverSidebarEntityStart(SidebarItem),
    HoverSidebarEntityEnd(SidebarItem),
    /// Right-click on a sidebar entity row — opens its context menu directly,
    /// without relying on hover state + the global uncaptured-right-click path.
    OpenSidebarEntityMenu(SidebarItem),
    /// Press-down on an album row's `mouse_area`. Begins a drag candidate (plain
    /// press), toggles the multi-selection (Cmd), or opens the menu (Ctrl). The
    /// actual navigate/drop is resolved on `MouseReleased`.
    AlbumPressed(AlbumId),
    /// Cursor entered / left a shelf header — tracks the album-drag drop target.
    HoverShelfStart(ShelfId),
    HoverShelfEnd(ShelfId),
    ToggleAddToAlbumSubmenu,

    // — shelves (containers that group albums) —
    StartCreateShelf,
    CreateShelfInputChanged(String),
    ConfirmCreateShelf,
    ShelfCreated,
    StartRenameShelf(ShelfId),
    RenameShelfInputChanged(String),
    ConfirmRenameShelf,
    ShelfRenamed,
    RequestDeleteShelf(ShelfId),
    CancelDeleteShelf,
    DeleteShelf(ShelfId),
    ShelfDeleted,
    ToggleShelfCollapsed(ShelfId),
    /// Left-press on a shelf header: Ctrl held → open its context menu (the
    /// right-click alias), otherwise toggle the shelf collapsed.
    ShelfHeaderPressed(ShelfId),
    /// Right-click / Ctrl+Click on a shelf header opens its context menu.
    OpenShelfMenu(ShelfId),
    /// File several albums at once (multi-select / drag), or `None` to ungroup.
    MoveAlbumsToShelf { album_ids: Vec<AlbumId>, shelf_id: Option<ShelfId> },
    /// Open the inline create-shelf input, filing `album_ids` into the new shelf
    /// once it's confirmed ("New Shelf…" chosen for a selection).
    StartCreateShelfFor(Vec<AlbumId>),
    /// Open the inline create-album input directly under a shelf, filing the new
    /// album there on confirm ("New Album" from a shelf's context menu).
    StartCreateAlbumIn(ShelfId),
    /// Select every album filed under a shelf (the shelf menu's "Select Albums",
    /// and what `Cmd+A` expands to when an album in that shelf is selected).
    SelectShelfAlbums(ShelfId),
    AlbumMovedToShelf,

    /// Wraps a context-menu leaf action: closes the menu, then dispatches the
    /// inner message. Every clickable menu item routes through this, so closing
    /// is handled in exactly one place (no per-handler `context_menu = None`).
    MenuAction(Box<Msg>),
    LoupeFullResLoaded { idx: usize, handle: iced::widget::image::Handle },
    /// Full-res decode failed for `idx` — surface the reason in the loupe.
    LoupeFullResFailed { idx: usize, error: crate::app::LoupeLoadError },
    /// Open the OS privacy settings so the user can grant file access (macOS).
    OpenPrivacySettings,
    LoupeHiresLoaded { idx: usize, handle: iced::widget::image::Handle },
    LoupePrefetchLoaded { idx: usize, handle: iced::widget::image::Handle },
    ThumbnailCompleted { file_id: String, path: String },
    ThumbnailFailed { file_id: String },
    FileWatcherEvent(isomfolio_core::indexing::types::FileEvent),
    FlushFileEvents(u64),
    SyncXmpForSelection,
    SyncAppleTagsForSelection,
    RequestRemoveMissing(String),
    ConfirmRemoveMissing,
    CancelRemoveMissing,
    /// Soft-delete the current grid selection (move to the virtual Deleted folder).
    DeleteSelection,
    /// Restore the current selection from the Deleted view.
    RestoreSelection,
    RequestDeleteRejects,
    ConfirmDeleteRejects,
    CancelDeleteRejects,
    SelectionDeleted,
    /// Move to OS Trash + drop catalog rows: the current selection / all in Deleted.
    RequestPurgeSelected,
    RequestPurgeAll,
    ConfirmPurge,
    CancelPurge,
    Purged(usize),
    LocateFile(String),
    FileLocated { file_id: String, new_path: std::path::PathBuf },

    /// Write XMP sidecars for the current selection (catalog → file, never
    /// touching the original image).
    WriteXmpSidecars,
    SidecarsWritten { count: usize, failed: usize },
    /// Export catalog metadata (selection or current view) to a CSV file.
    ExportMetadata,
    ExportMetadataDest { ids: Vec<String>, dest: Option<String> },
    MetadataExported,
    ExportSelectionToDialog(ExportMode),
    /// Copy every (present) file in an album into a sub-folder named after the
    /// album, under a destination the user picks.
    ExportAlbumToDialog(String),
    /// Copy every album on a shelf, mirroring the structure as
    /// `<dest>/<shelf>/<album>/…`.
    ExportShelfToDialog(ShelfId),
    ExportDestPicked { entries: Vec<CopyEntry>, dest: Option<String>, mode: ExportMode },
    ExportDone { task_id: BgTaskId, result: Result<(), String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportMode {
    Copy,
}

/// One file to copy plus the (already-sanitised) sub-folder path it should land
/// in, relative to the chosen destination root. `rel` empty = straight into the
/// destination; `["Shelf", "Album"]` = `<dest>/Shelf/Album/<file>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyEntry {
    pub rel: Vec<String>,
    pub src: String,
}

use isomfolio_core::models::FlagSelection;

pub enum UndoOp {
    AddedTag { file_ids: Vec<String>, tag: String },
    RemovedTag { file_ids: Vec<String>, tag: String },
    SetRatings { before: Vec<(String, Option<i32>)> },
    SetFlags { before: Vec<(String, Flag)> },
    SetLabels { before: Vec<(String, Option<String>)> },
}

/// Comparator for the star-rating filter UI. Combined with a star count chip to
/// produce a `RatingFilter::{AtLeast,Exactly,AtMost}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatingCmp {
    AtLeast,
    Exactly,
    AtMost,
}

impl RatingCmp {
    pub fn apply(self, n: i32) -> isomfolio_core::models::RatingFilter {
        use isomfolio_core::models::RatingFilter;
        match self {
            RatingCmp::AtLeast => RatingFilter::AtLeast(n),
            RatingCmp::Exactly => RatingFilter::Exactly(n),
            RatingCmp::AtMost => RatingFilter::AtMost(n),
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            RatingCmp::AtLeast => "≥",
            RatingCmp::Exactly => "=",
            RatingCmp::AtMost => "≤",
        }
    }
}

/// Editable descriptive metadata field in the detail panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailField {
    Title,
    Caption,
    Creator,
    Rights,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatePreset {
    Last7,
    Last30,
    ThisMonth,
    ThisYear,
}

pub struct FilterState {
    pub tags: Vec<String>,
    pub tag_input: String,
    /// How include `tags` combine (AND/OR).
    pub tag_match: isomfolio_core::models::TagMatch,
    /// Tags that exclude a file (NOT). Rendered in the same chip row, struck-through.
    pub exclude_tags: Vec<String>,
    pub date_from: String,
    pub date_to: String,
    pub exts: HashSet<String>,
    pub save_smart_input: Option<String>,
    pub flags: FlagSelection,
    pub rating: isomfolio_core::models::RatingFilter,
    /// UI-only: comparator applied to the next star-count chip click.
    pub rating_cmp: RatingCmp,
    pub has_location: Option<bool>,
    /// Selected person face-cluster id, if filtering by person.
    pub person: Option<String>,
    /// "Added recently" window in days (filters on catalog add time); None = any.
    pub added_within_days: Option<i64>,
    /// Selected EXIF camera model, if filtering by camera.
    pub camera: Option<String>,
    /// Selected colour label, if filtering by colour.
    pub color: Option<String>,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            tags: Vec::new(),
            tag_input: String::new(),
            tag_match: isomfolio_core::models::TagMatch::All,
            exclude_tags: Vec::new(),
            date_from: String::new(),
            date_to: String::new(),
            exts: HashSet::new(),
            save_smart_input: None,
            flags: FlagSelection::default(),
            rating: isomfolio_core::models::RatingFilter::Any,
            rating_cmp: RatingCmp::AtLeast,
            has_location: None,
            person: None,
            added_within_days: None,
            camera: None,
            color: None,
        }
    }
}

pub struct DetailState {
    pub show: bool,
    pub file_id: Option<String>,
    pub batch_file_ids: Vec<String>,
    pub tags: Vec<String>,
    pub tag_input: String,
    pub all_tags: Vec<String>,
    pub recent_tags: Vec<String>,
    pub rating: Option<i32>,
    pub label: Option<String>,
    pub title: Option<String>,
    pub exif_tech: Option<isomfolio_core::models::ExifTechMeta>,
    /// Editable descriptive-metadata buffers (title/caption/creator/rights).
    pub title_input: String,
    pub caption_input: String,
    pub creator_input: String,
    pub rights_input: String,
}

const MAX_RECENT_TAGS: usize = 8;

impl DetailState {
    pub fn push_recent_tag(&mut self, tag: &str) {
        self.recent_tags.retain(|t| t != tag);
        self.recent_tags.insert(0, tag.to_string());
        self.recent_tags.truncate(MAX_RECENT_TAGS);
    }
}

impl Default for DetailState {
    fn default() -> Self {
        Self {
            show: false,
            file_id: None,
            batch_file_ids: Vec::new(),
            tags: Vec::new(),
            tag_input: String::new(),
            all_tags: Vec::new(),
            recent_tags: Vec::new(),
            rating: None,
            label: None,
            title: None,
            exif_tech: None,
            title_input: String::new(),
            caption_input: String::new(),
            creator_input: String::new(),
            rights_input: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Extensions,
}

pub struct SettingsState {
    pub tab: SettingsTab,
    /// extension_name -> key -> current edited value
    pub extension_configs: HashMap<String, HashMap<String, String>>,
    pub install_error: Option<String>,
    pub status: Option<String>,
    pub install_task_id: Option<BgTaskId>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            tab: SettingsTab::General,
            extension_configs: HashMap::new(),
            install_error: None,
            status: None,
            install_task_id: None,
        }
    }
}

pub type BgTaskId = u32;

#[derive(Debug, Clone)]
pub struct BgTask {
    pub id: BgTaskId,
    pub label: String,
    pub progress: Option<f32>,
    pub failed: Option<String>,
}

/// A finished task that lingers briefly in the panel with a ✓ before expiring,
/// so completion is visible app-wide instead of the row silently vanishing.
#[derive(Debug, Clone)]
pub struct CompletedTask {
    pub title: String,
    pub detail: String,
    pub at: std::time::Instant,
}

pub struct TagBrowserState {
    pub tags: Vec<(String, usize)>,
    pub filter: String,
    pub rename: Option<(String, String)>,
    pub delete_armed: Option<String>,
}

impl Default for TagBrowserState {
    fn default() -> Self {
        Self {
            tags: Vec::new(),
            filter: String::new(),
            rename: None,
            delete_armed: None,
        }
    }
}
