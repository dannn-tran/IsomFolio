use std::collections::{HashMap, HashSet};

use iced::{keyboard, Point};
use isomfolio_core::models::{Album, AlbumId, AssetFile, Flag};

#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    Folder(String),
    ManualAlbum(AlbumId),
    SmartAlbum(AlbumId),
    GridTiles,
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
pub const ALBUM_ITEM_HEIGHT: f32 = 44.0;
pub const FOLDER_ITEM_HEIGHT: f32 = 28.0;
pub const DRAG_THRESHOLD: f32 = 6.0;
pub const BUFFER_ROWS: usize = 2;
pub const SIDEBAR_ALBUMS_BASE_Y: f32 = 184.0;
pub const SEARCH_BAR_HEIGHT: f32 = 40.0;
pub const CRITERIA_ROW_HEIGHT: f32 = 32.0;
pub const CRITERIA_ROW_COUNT: usize = 5;
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
    ConfirmRemoveFromAlbum,
    CancelRemoveFromAlbum,

    SortFieldCycle,
    SortDirToggle,

    SearchChanged(String),

    ToggleCriteria,
    CriteriaTagInputChanged(String),
    AddCriteriaTag,
    RemoveCriteriaTag(String),
    CriteriaDateFromChanged(String),
    CriteriaDateToChanged(String),
    ToggleCriteriaExt(String),
    ClearCriteria,

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
    },
    DetailTagInputChanged(String),
    AddDetailTag,
    RemoveDetailTag(String),
    SetDetailRating(i32),
    AllTagsLoaded(Vec<String>),
    AddDetailTagDirect(String),

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
    Tick,
    DragHoverAlbum(Option<AlbumId>),
    SidebarScrolled(f32),

    PickOpenCatalog,
    OpenCatalogPicked(String),
    SelectRecentCatalog(String),
    OpenSelectedRecentCatalog,
    ShowNewCatalogModal,
    HideNewCatalogModal,
    PickNewCatalogDir,
    NewCatalogDirPicked(String),
    NewCatalogNameChanged(String),
    ConfirmNewCatalog,
    OpenCatalog(String),

    RequestDeleteAlbum(AlbumId),
    CancelDeleteAlbum,
    RequestRemoveFolder(String),
    CancelRemoveFolder,

    ScanDialogDone(Option<String>),
    SetFlag(Flag),
    SetRating(Option<i32>),
    FlagsApplied,
    RatingsApplied,
    RatingsLoaded(HashMap<String, i32>),
    ToggleHideRejects,
    SetFlagFilter(FlagFilter),
    SetRatingFilter(Option<i32>),

    SortCycleAll,
    NoOp,

    SidebarResizeStart,
    OpenContextMenu(Point, ContextMenuTarget),
    CloseContextMenu,
    RescanFolder(String),
    DuplicateAlbum(AlbumId),
    ShowInFinder(String),
    AddSelectionToAlbum(AlbumId),
    HoverSidebarEntityStart(SidebarItem),
    HoverSidebarEntityEnd(SidebarItem),
    ToggleAddToAlbumSubmenu,
    LoupeFullResLoaded { idx: usize, handle: iced::widget::image::Handle },
    LoupePrefetchLoaded { idx: usize, handle: iced::widget::image::Handle },
    ThumbnailHandleReady { file_id: String, handle: iced::widget::image::Handle },
}

use isomfolio_core::models::FlagFilter;

pub struct CriteriaState {
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
            flag_filter: FlagFilter::All,
            rating_min: None,
            hide_rejects: false,
        }
    }
}

pub struct DetailState {
    pub show: bool,
    pub file_id: Option<String>,
    pub tags: Vec<String>,
    pub tag_input: String,
    pub all_tags: Vec<String>,
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
            all_tags: Vec::new(),
            rating: None,
            label: None,
            title: None,
        }
    }
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
