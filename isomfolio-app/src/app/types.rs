use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use std::sync::LazyLock;
use iced::{keyboard, widget, Point};

pub static GRID_SCROLL_ID: LazyLock<widget::Id> = LazyLock::new(|| widget::Id::unique());
use isomfolio_core::extension::{ExtensionManifest, ExtensionProcess};
use isomfolio_core::models::{Album, AlbumId, AssetFile, Flag};

#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    Folder(String),
    ManualAlbum(AlbumId),
    SmartAlbum(AlbumId),
    GridTiles,
    FaceCluster(String),
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
pub const ALBUM_ITEM_HEIGHT: f32 = 32.0;
pub const FOLDER_ITEM_HEIGHT: f32 = 28.0;
pub const DRAG_THRESHOLD: f32 = 6.0;
pub const TILE_SIZE_STEP: f32 = 40.0;
pub const TILE_SIZE_MIN: f32 = 80.0;
pub const TILE_SIZE_MAX: f32 = 400.0;
pub const SIDEBAR_WIDTH_MIN: f32 = 140.0;
pub const SIDEBAR_WIDTH_MAX: f32 = 400.0;
pub const ALBUM_ROW_GAP: f32 = 2.0;
pub const BUFFER_ROWS: usize = 2;
pub const SIDEBAR_ALBUMS_BASE_Y: f32 = 184.0;
pub const SEARCH_BAR_HEIGHT: f32 = 40.0;
pub const CRITERIA_ROW_HEIGHT: f32 = 32.0;
pub const CRITERIA_ROW_COUNT: usize = 5;
pub const CRITERIA_PADDING: f32 = 18.0;

#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    Browse,
    Preview,
    Loupe,
    People,
    Compare,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SuggestionView {
    Photo,
    Tag,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SidebarItem {
    AllFiles,
    Folder(String),
    Album(AlbumId),
    FaceCluster(String),
    Suggestions,
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
    Failed(String),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Msg {
    CatalogReady,

    SidebarItemClicked(SidebarItem),

    FilesLoaded(Vec<AssetFile>),
    SidebarLoaded {
        folders: Vec<(String, String, usize)>,
        albums: Vec<Album>,
        album_counts: HashMap<String, usize>,
    },

    TileSizeUp,
    TileSizeDown,

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
    OpenLoupe,

    Scrolled {
        y: f32,
        height: f32,
        width: f32,
    },

    DroppedToAlbum(AlbumId, Vec<String>),
    DropCompleted,

    SyncPickFolder,
    SyncStart(String),
    SyncComplete { count: usize, new_file_ids: Vec<String> },

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
    ConfirmRemoveFromAlbum,
    CancelRemoveFromAlbum,

    SortFieldCycle,
    SortDirToggle,

    SearchChanged(String),

    ToggleFilterPanel,
    FilterTagInputChanged(String),
    AddFilterTag,
    RemoveFilterTag(String),
    FilterDateFromChanged(String),
    FilterDateToChanged(String),
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
        tag_confidence: HashMap<String, f32>,
        pending_tags: Vec<(String, Option<f32>)>,
        rating: Option<i32>,
        label: Option<String>,
        title: Option<String>,
        exif_tech: Option<isomfolio_core::models::ExifTechMeta>,
    },
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
    AcceptPendingTag(String),
    RejectPendingTag(String),
    AcceptAllPending,
    RejectAllPending,
    PendingTagsUpdated,
    AcceptAllInView,
    RejectAllInView,
    PendingCountsLoaded { counts: HashMap<String, usize>, total: usize },
    PendingTotalLoaded(usize),
    SetSuggestionView(SuggestionView),
    PendingTagGroupsLoaded(Vec<isomfolio_core::models::PendingTagGroup>),
    AcceptPendingTagGlobally(String),
    RejectPendingTagGlobally(String),

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
    DragHoverAlbum(Option<AlbumId>),
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
    FlagsApplied,
    RatingsApplied,
    RatingsLoaded(HashMap<String, i32>),
    ToggleHideRejects,
    SetFlagFilter(FlagFilter),
    SetRatingFilter(Option<i32>),
    SetLocationFilter(Option<bool>),

    ExtensionsDiscovered(Vec<Arc<ExtensionProcess>>, Option<ExtensionManifest>),
    RunExtension { addon_idx: usize, method: String, file_ids: Vec<String> },
    ExtensionProgress { addon_idx: usize, file_id: String, percent: u8 },
    ExtensionBatchProgress { name: String, done: usize, total: usize },
    ExtensionBatchDone { addon_idx: usize, method: String, applied: usize, failed: usize },
    ExtensionRestarted { idx: usize, process: Option<Arc<ExtensionProcess>> },

    BgTaskDismissed(BgTaskId),
    ToggleTaskPanel,

    OpenSettings,
    SwitchSettingsTab(SettingsTab),
    CloseSettings,
    ToggleAutoFaceCluster,
    ToggleImportXmpTags,
    ToggleImportAppleTags,
    ToggleAutoAdvanceOnFlag,
    SettingsConfigChanged { extension_name: String, key: String, value: String },
    SaveSettings,
    InstallExtensionPickFile,
    ExtensionPackagePicked(Option<String>),
    ExtensionInstalled(Arc<ExtensionProcess>),
    EngineInstalled(ExtensionManifest),
    ExtensionInstallFailed(String),
    UninstallExtension(String),
    SetPreferredExtension { capability: String, extension_name: String },

    RunFaceClustering { force_full: bool },
    InferenceEngineReady {
        client: Result<Arc<crate::inference::InferenceClient>, String>,
        force_full: bool,
    },
    FaceClusteringDone(Vec<isomfolio_core::models::FaceClusterSummary>),
    FaceClustersBatchDone(Vec<isomfolio_core::models::FaceClusterSummary>),
    FaceClustersLoaded(Vec<isomfolio_core::models::FaceClusterSummary>),
    RenameFaceCluster(String),
    RenameFaceClusterInputChanged(String),
    ConfirmRenameFaceCluster,
    MergeFaceClusters(String, String),
    RemoveFileFromFaceCluster(String, String),
    FaceCropsReady(Vec<(String, iced::widget::image::Handle)>),
    OpenPeopleView,

    SelectAll,
    DeselectAll,
    OpenFaceClusterMenu(String),
    Undo,
    Redo,
    UndoApplied,
    OpenCompare,
    CompareFullResLoaded { slot: usize, handle: iced::widget::image::Handle },
    SortCycleAll,
    NoOp,

    SidebarResizeStart,
    OpenContextMenu(Point, ContextMenuTarget),
    CloseContextMenu,
    SyncFolder(String),
    DuplicateAlbum(AlbumId),
    ShowInFinder(Vec<String>),
    AddSelectionToAlbum(AlbumId),
    HoverSidebarEntityStart(SidebarItem),
    HoverSidebarEntityEnd(SidebarItem),
    ToggleAddToAlbumSubmenu,
    LoupeFullResLoaded { idx: usize, handle: iced::widget::image::Handle },
    LoupePrefetchLoaded { idx: usize, handle: iced::widget::image::Handle },
    ThumbnailHandleReady { file_id: String, handle: iced::widget::image::Handle },
    ThumbnailCompleted { file_id: String, path: String },
    ThumbnailFailed { file_id: String },
    FileWatcherEvent(isomfolio_core::indexing::types::FileEvent),
    FlushFileEvents(u64),
    SyncXmpForSelection,
    SyncAppleTagsForSelection,
    MetadataImportPromptToggleXmp,
    MetadataImportPromptToggleApple,
    MetadataImportPromptToggleAll,
    MetadataImportPromptContinue,
    MetadataImportPromptCancel,
    RequestRemoveMissing(String),
    ConfirmRemoveMissing,
    CancelRemoveMissing,
    LocateFile(String),
    FileLocated { file_id: String, new_path: std::path::PathBuf },

    ExportSelectionToDialog(ExportMode),
    ExportDestPicked { paths: Vec<String>, dest: Option<String>, mode: ExportMode },
    ExportDone { task_id: BgTaskId, result: Result<(), String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportMode {
    Copy,
    Move,
}

use isomfolio_core::models::FlagFilter;

pub enum UndoOp {
    AddedTag { file_ids: Vec<String>, tag: String },
    RemovedTag { file_ids: Vec<String>, tag: String },
    SetRatings { before: Vec<(String, Option<i32>)> },
    SetFlags { before: Vec<(String, Flag)> },
}

pub struct FilterState {
    pub show: bool,
    pub tags: Vec<String>,
    pub tag_input: String,
    pub date_from: String,
    pub date_to: String,
    pub exts: HashSet<String>,
    pub save_smart_input: Option<String>,
    pub flag_filter: FlagFilter,
    pub rating_min: Option<i32>,
    pub hide_rejects: bool,
    pub has_location: Option<bool>,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            show: false,
            tags: Vec::new(),
            tag_input: String::new(),
            date_from: String::new(),
            date_to: String::new(),
            exts: HashSet::new(),
            save_smart_input: None,
            flag_filter: FlagFilter::All,
            rating_min: None,
            hide_rejects: false,
            has_location: None,
        }
    }
}

pub struct DetailState {
    pub show: bool,
    pub file_id: Option<String>,
    pub batch_file_ids: Vec<String>,
    pub tags: Vec<String>,
    pub tag_confidence: HashMap<String, f32>,
    pub pending_tags: Vec<(String, Option<f32>)>,
    pub tag_input: String,
    pub all_tags: Vec<String>,
    pub recent_tags: Vec<String>,
    pub rating: Option<i32>,
    pub label: Option<String>,
    pub title: Option<String>,
    pub exif_tech: Option<isomfolio_core::models::ExifTechMeta>,
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
            tag_confidence: HashMap::new(),
            pending_tags: Vec::new(),
            tag_input: String::new(),
            all_tags: Vec::new(),
            recent_tags: Vec::new(),
            rating: None,
            label: None,
            title: None,
            exif_tech: None,
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
