module IsomFolio.UI.MainView

open Elmish
open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
open Avalonia.Threading
open IsomFolio.Core.Indexing.Types
open IsomFolio.Core.Models
open IsomFolio.Core.FileIndex
open IsomFolio.Core.Indexing
open IsomFolio.Core.Metadata
open IsomFolio.Core.Storage
open IsomFolio.Core.Search
open IsomFolio.Core.PathUtils

let mutable private thumbnailWorker: MailboxProcessor<Thumbnail.ThumbnailMsg> option = None
let mutable private activeWatchers: System.IO.FileSystemWatcher list = []
let mutable private appWindow: Window option = None

type CatalogState =
    | Unloaded
    | OpenedCatalog of catalogPath: string

type State = {
    Sidebar         : Sidebar.State
    Grid            : GridView.State
    Detail          : DetailPanel.State
    SearchBar       : SearchBar.State
    ScanProgress    : ScanProgress option
    ActiveQuery     : SearchQuery
    Notifications   : (string * System.DateTime) list
    OrphanCount     : int
    IsFirstRun      : bool
    Catalog         : CatalogState
    SearchRequestId : int
    PendingFolders  : Set<string>
    TagBrowser      : TagBrowser.State option
}

type Msg =
    | SidebarMsg          of Sidebar.Msg
    | GridMsg             of GridView.Msg
    | DetailMsg           of DetailPanel.Msg
    | SearchBarMsg        of SearchBar.Msg
    | TagBrowserMsg       of TagBrowser.Msg
    | FolderOpened        of folderPath: string
    | ScanProgressUpdated of ScanProgress
    | ScanBatchCompleted  of AssetFile list
    | ScanFinished        of totalCount: int
    | SearchCompleted     of requestId: int * files: AssetFile list
    | FileEventReceived        of FileEvent
    | ResyncFolderRequested    of folderPath: string
    | FolderResynced           of folderPath: string
    | TagsUpdated              of FileId * string list
    | AppError            of AppError
    | OrphanCountLoaded   of int
    | DismissNotification of System.DateTime
    | AddFolderRequested
    | FolderRemoveRequested of folderPath: string
    | NewCatalogRequested
    | OpenCatalogRequested
    | CatalogOpened       of catalogPath: string * folders: string list
    | NoOp

let private defaultQuery = {
    Text = None; FolderPath = None; Tags = []; Extensions = []
    DateRange = None; SortBy = Date; SortAsc = false
}

let init (w: Window) () : State * Cmd<Msg> =
    appWindow <- Some w
    let state = {
        Sidebar         = Sidebar.init ()
        Grid            = GridView.init ()
        Detail          = DetailPanel.init ()
        SearchBar       = SearchBar.init ()
        ScanProgress    = None
        ActiveQuery     = defaultQuery
        Notifications   = []
        OrphanCount     = 0
        IsFirstRun      = true
        Catalog         = Unloaded
        SearchRequestId = 0
        PendingFolders  = Set.empty
        TagBrowser      = None
    }
    let initCmd =
        match IsomFolio.Core.AppPaths.readLastSession() with
        | None -> Cmd.none
        | Some session ->
            Cmd.OfAsync.either
                (fun () -> async {
                    let! conn = Db.openDatabase (IsomFolio.Core.AppPaths.dbPath session.CatalogPath)
                    use c = conn
                    return session.CatalogPath, session.Folders
                })
                ()
                CatalogOpened
                (fun ex -> AppError (DbError ex.Message))
    state, initCmd

let private withCatalogDb (catalogPath: string) (work: _ -> Async<'T>) : Async<'T> =
    async {
        let! conn = Db.openDatabase (IsomFolio.Core.AppPaths.dbPath catalogPath)
        use c = conn
        return! work c
    }

let private runSearch (catalogPath: string) (requestId: int) (query: SearchQuery) : Cmd<Msg> =
    Cmd.OfAsync.either
        (fun () -> withCatalogDb catalogPath (fun c -> query |> QueryEngine.executeSearch c))
        ()
        (fun files -> SearchCompleted(requestId, files))
        (fun ex -> AppError (DbError ex.Message))

let private isFilterActive (query: SearchQuery) =
    query.Text |> Option.exists (fun txt -> txt.Trim() <> "")
    || query.FolderPath.IsSome
    || not query.Tags.IsEmpty
    || not query.Extensions.IsEmpty
    || query.DateRange.IsSome

let private stopFolderWatcherCmd (folderPath: string) : Cmd<Msg> =
    Cmd.ofEffect (fun _ ->
        let toStop, remaining =
            activeWatchers
            |> List.partition (fun w -> IsomFolio.Core.PathUtils.samePath w.Path folderPath)
        for w in toStop do Watcher.stopWatcher w
        activeWatchers <- remaining)

let private removeFolderFilesAndSearchCmd (catalogPath: string) (folderPath: string) (requestId: int) (query: SearchQuery) : Cmd<Msg> =
    Cmd.OfAsync.either
        (fun () ->
            withCatalogDb catalogPath (fun c ->
                async {
                    do! Db.deleteFilesByRootFolder c folderPath
                    return! QueryEngine.executeSearch c query
                }))
        ()
        (fun files -> SearchCompleted(requestId, files))
        (fun ex -> AppError (DbError ex.Message))

let private confirmFolderRemovalCmd (folderPath: string) : Cmd<Msg> =
    Cmd.ofEffect (fun dispatch ->
        match appWindow with
        | None -> ()
        | Some owner ->
            let folderName = System.IO.Path.GetFileName(folderPath)
            let dialog = new Window(
                Title = "Remove Folder",
                Width = 380.0,
                SizeToContent = SizeToContent.Height,
                WindowStartupLocation = WindowStartupLocation.CenterOwner,
                CanResize = false,
                ShowInTaskbar = false)

            let panel = StackPanel(Margin = Avalonia.Thickness(20.0), Spacing = 16.0)

            let label =
                TextBlock(
                    Text = $"Remove \"{folderName}\" from the library?\n\nFiles on disk are not affected. Images from this folder will no longer appear in the grid.",
                    TextWrapping = Avalonia.Media.TextWrapping.Wrap)

            let btnRow = new StackPanel(
                Orientation = Orientation.Horizontal,
                HorizontalAlignment = HorizontalAlignment.Right,
                Spacing = 8.0)

            let cancelBtn = Button(Content = "Cancel", IsCancel = true)
            let removeBtn = Button(Content = "Remove", IsDefault = true)

            cancelBtn.Click.Add(fun _ -> dialog.Close(false))
            removeBtn.Click.Add(fun _ -> dialog.Close(true))

            btnRow.Children.Add(cancelBtn)
            btnRow.Children.Add(removeBtn)
            panel.Children.Add(label)
            panel.Children.Add(btnRow)
            dialog.Content <- panel

            dialog.ShowDialog<bool>(owner)
                .ContinueWith(fun (t: System.Threading.Tasks.Task<bool>) ->
                    if not t.IsFaulted && not t.IsCanceled && t.Result then
                        Dispatcher.UIThread.Post(fun () ->
                            dispatch (SidebarMsg (Sidebar.FolderRemoved folderPath))))
            |> ignore)

let private manageWorkerCmd (catalogPath: string) : Cmd<Msg> =
    Cmd.ofEffect (fun dispatch ->
        // 1. Shutdown existing resources
        for w in activeWatchers do Watcher.stopWatcher w
        activeWatchers <- []
        thumbnailWorker |> Option.iter (fun w -> w.Post(Thumbnail.Shutdown))
        thumbnailWorker <- None
        GridView.clearBitmapCache()

        // 2. Start new worker pool
        let worker =
            Thumbnail.createWorkerPool catalogPath 4
                (fun fileId path ->
                    Dispatcher.UIThread.Post(fun () ->
                        dispatch (GridMsg (GridView.ThumbnailUpdated(fileId, Ready path)))))
                (fun fileId _ ->
                    Dispatcher.UIThread.Post(fun () ->
                        dispatch (GridMsg (GridView.ThumbnailUpdated(fileId, Failed 2)))))
        thumbnailWorker <- Some worker)

let private startScanCmd (catalogPath: string) (folderPath: string) : Cmd<Msg> =
    Cmd.ofEffect (fun dispatch ->
        Async.Start(async {
            let! result =
                withCatalogDb catalogPath (fun dbConn ->
                    Scanner.scanFolder folderPath
                        (fun batch -> async {
                            let assets = batch |> List.map (fun sf -> sf.Asset)
                            let! _ = Db.upsertFiles dbConn assets
                            for sf in batch do
                                do! Db.upsertMetadata dbConn sf.Asset.Id sf.Metadata
                            Dispatcher.UIThread.Post(fun () -> dispatch (ScanBatchCompleted assets))
                        })
                        (fun progress ->
                            Dispatcher.UIThread.Post(fun () -> dispatch (ScanProgressUpdated progress))))
            Dispatcher.UIThread.Post(fun () -> dispatch (ScanFinished result.TotalCount))
        }))

let private createWatcherCmd (folderPath: string) : Cmd<Msg> =
    Cmd.ofEffect (fun dispatch ->
        let w = Watcher.createWatcher folderPath (fun event ->
            Dispatcher.UIThread.Post(fun () -> dispatch (FileEventReceived event)))
        activeWatchers <- w :: activeWatchers)


let private loadFolderTreeCmd (folders: string list) : Cmd<Msg> =
    Cmd.OfAsync.either
        (fun () -> async { return FolderTree.buildForest folders })
        ()
        (Sidebar.FolderTreeLoaded >> SidebarMsg)
        (fun ex -> AppError (ScanError ex.Message))

let private normalizeFolders (folders: string list) =
    folders
    |> List.map IsomFolio.Core.PathUtils.normalizePath
    |> List.fold (fun acc path ->
        if acc |> List.exists (fun existing -> IsomFolio.Core.PathUtils.samePath existing path) then acc
        else acc @ [ path ]) []


let private applyReconcileResult (dbConn: Microsoft.Data.Sqlite.SqliteConnection) (result: ReconcileResult) =
    async {
        for oid in result.Orphaned do
            do! Db.markOrphaned dbConn oid
        let! scanned = Scanner.refreshMetadata result.NewOrModified
        for path, meta in scanned do
            try
                let fi = System.IO.FileInfo(path)
                let asset = assetFileFromInfo fi
                let! _ = Db.upsertFiles dbConn [ asset ]
                do! Db.upsertMetadata dbConn asset.Id meta
            with ex ->
                eprintfn "Reconcile: cannot index %s — %s" path ex.Message
        let! sidecarMeta = Scanner.refreshMetadata result.SidecarChanged
        for path, meta in sidecarMeta do
            let fileId = IsomFolio.Core.FileIndex.computeFileId (IsomFolio.Core.PathUtils.normalizePath path)
            do! Db.upsertMetadata dbConn fileId meta
    }

let private reconcileFolderCmd (catalogPath: string) (folderPath: string) : Cmd<Msg> =
    Cmd.OfAsync.either
        (fun () ->
            withCatalogDb catalogPath (fun dbConn ->
                async {
                    let! result = Scanner.reconcileFolder dbConn folderPath
                    do! applyReconcileResult dbConn result
                }))
        ()
        (fun () -> ScanFinished 0)
        (fun ex -> AppError (ScanError ex.Message))

let private resyncFolderCmd (catalogPath: string) (folderPath: string) : Cmd<Msg> =
    Cmd.OfAsync.either
        (fun () ->
            withCatalogDb catalogPath (fun dbConn ->
                async {
                    let! result = Scanner.reconcileFolder dbConn folderPath
                    do! applyReconcileResult dbConn result
                }))
        ()
        (fun () -> FolderResynced folderPath)
        (fun ex -> AppError (ScanError ex.Message))

let private startupCleanupCmd (catalogPath: string) : Cmd<Msg> =
    Cmd.OfAsync.either
        (fun () ->
            withCatalogDb catalogPath (fun conn ->
                async {
                    let! _ = Db.purgeOldOrphans conn 30
                    let! _ = Thumbnail.sweepThumbnailCache conn catalogPath
                    let! n = Db.countOrphans conn
                    return n
                }))
        ()
        OrphanCountLoaded
        (fun ex -> AppError (DbError ex.Message))

let private countOrphansCmd (catalogPath: string) : Cmd<Msg> =
    Cmd.OfAsync.either
        (fun () -> withCatalogDb catalogPath Db.countOrphans) ()
        OrphanCountLoaded
        (fun ex -> AppError (DbError ex.Message))

let private loadMetadataCmd (catalogPath: string) (fileId: FileId) : Cmd<Msg> =
    Cmd.OfAsync.either
        (fun () -> withCatalogDb catalogPath (fun c -> Db.getMetadata c fileId))
        ()
        (fun meta -> DetailMsg (DetailPanel.MetadataLoaded meta))
        (fun _    -> NoOp)

let private loadSourceViewCmd (filePath: string) (fileId: FileId) : Cmd<Msg> =
    Cmd.OfAsync.either
        (fun () -> async {
            let fi = System.IO.FileInfo(filePath)
            return! EmbeddedMetadata.readSources filePath fi
        })
        ()
        (fun sources -> DetailMsg (DetailPanel.SourceViewLoaded sources))
        (fun ex      -> DetailMsg (DetailPanel.SourceViewFailed ex))

let private formatError = function
    | DbError msg              -> $"Database error: {msg}"
    | ScanError msg            -> $"Scan error: {msg}"
    | ThumbnailError(_, msg)   -> $"Thumbnail failed: {msg}"
    | WatcherError msg         -> $"Watcher error: {msg}"

let private enqueueThumbnails (catalogPath: string) (files: AssetFile list) (priority: int) =
    match thumbnailWorker with
    | Some w ->
        for f in files do
            w.Post(Thumbnail.Enqueue({ FileId = f.Id; FilePath = f.Path; Priority = priority }, 0))
    | None -> ()

let private primeGridThumbnails (catalogPath: string) (priority: int) (grid: GridView.State) =
    let pendingFiles = System.Collections.Generic.List<AssetFile>()
    let updatedStates = System.Collections.Generic.Dictionary<FileId, ThumbnailState>()

    for tile in grid.Tiles do
        match tile.Thumbnail with
        | NotRequested ->
            let cachePath = Thumbnail.thumbnailCachePath catalogPath tile.File.Id
            if Thumbnail.isCacheValid catalogPath tile.File.Id then
                updatedStates[tile.File.Id] <- Ready cachePath
            else
                updatedStates[tile.File.Id] <- Pending
                pendingFiles.Add(tile.File)
        | _ -> ()

    if pendingFiles.Count > 0 then
        enqueueThumbnails catalogPath (pendingFiles |> Seq.toList) priority

    if updatedStates.Count = 0 then grid
    else
        let newTiles =
            grid.Tiles |> List.map (fun t ->
                match updatedStates.TryGetValue(t.File.Id) with
                | true, state -> { t with Thumbnail = state }
                | _ -> t)
        { grid with Tiles = newTiles }

let private uiDispatch (dispatch: Msg -> unit) (msg: Msg) =
    Dispatcher.UIThread.Post(fun () -> dispatch msg)

let private handleCatalogMsg (state: State) (msg: Msg) : (State * Cmd<Msg>) option =
    appWindow |> Option.bind (fun w ->
        match msg with
        | NewCatalogRequested ->
            let cmd = Cmd.ofEffect (fun dispatch ->
                let opts = Avalonia.Platform.Storage.FilePickerSaveOptions(
                    Title = "New Catalog",
                    SuggestedFileName = "my-library",
                    DefaultExtension = "isomfolio")
                w.StorageProvider.SaveFilePickerAsync(opts)
                    .ContinueWith(fun (t: System.Threading.Tasks.Task<Avalonia.Platform.Storage.IStorageFile>) ->
                        if t.IsFaulted then
                            uiDispatch dispatch (AppError (ScanError (t.Exception.GetBaseException().Message)))
                        elif not t.IsCanceled && not (isNull t.Result) then
                            let rawPath = t.Result.Path.LocalPath
                            let parentDir = System.IO.Path.GetDirectoryName(rawPath)
                            let baseName  = System.IO.Path.GetFileNameWithoutExtension(rawPath)
                            Async.Start(async {
                                try
                                    let catalogPath = IsomFolio.Core.AppPaths.createCatalog parentDir baseName
                                    let! conn = Db.openDatabase (IsomFolio.Core.AppPaths.dbPath catalogPath)
                                    use c = conn
                                    uiDispatch dispatch (CatalogOpened (catalogPath, []))
                                with ex ->
                                    uiDispatch dispatch (AppError (ScanError ex.Message)) }))
                |> ignore)
            Some(state, cmd)
        | OpenCatalogRequested ->
            let cmd = Cmd.ofEffect (fun dispatch ->
                let opts = Avalonia.Platform.Storage.FolderPickerOpenOptions(
                    Title = "Open Catalog",
                    AllowMultiple = false)
                w.StorageProvider.OpenFolderPickerAsync(opts)
                    .ContinueWith(fun (t: System.Threading.Tasks.Task<System.Collections.Generic.IReadOnlyList<Avalonia.Platform.Storage.IStorageFolder>>) ->
                        if t.IsFaulted then
                            uiDispatch dispatch (AppError (DbError (t.Exception.GetBaseException().Message)))
                        elif not t.IsCanceled && t.Result.Count > 0 then
                            let catalogPath = t.Result[0].Path.LocalPath
                            Async.Start(async {
                                try
                                    let! conn = Db.openDatabase (IsomFolio.Core.AppPaths.dbPath catalogPath)
                                    use c = conn
                                    let folders =
                                        match IsomFolio.Core.AppPaths.readLastSession() with
                                        | Some s when s.CatalogPath = catalogPath -> s.Folders
                                        | _ -> []
                                    uiDispatch dispatch (CatalogOpened (catalogPath, folders))
                                with ex ->
                                    uiDispatch dispatch (AppError (DbError ex.Message)) }))
                |> ignore)
            Some(state, cmd)
        | CatalogOpened (path, folders) ->
            let folders = normalizeFolders folders
            IsomFolio.Core.AppPaths.saveSession { CatalogPath = path; Folders = folders }
            let perFolderCmds =
                folders |> List.collect (fun f -> [ reconcileFolderCmd path f; createWatcherCmd f ])
            let scanProgress =
                if folders.IsEmpty then None
                else Some { TotalFound = 0; Inserted = 0; FolderName = "Restoring…" }
            Some(
                { state with
                    Catalog         = OpenedCatalog(path)
                    IsFirstRun      = false
                    Sidebar         = Sidebar.update (Sidebar.FoldersLoaded folders) state.Sidebar
                    Grid            = GridView.init ()
                    Detail          = DetailPanel.init ()
                    SearchBar       = SearchBar.init ()
                    ActiveQuery     = defaultQuery
                    ScanProgress    = scanProgress
                    PendingFolders  = Set.empty
                    TagBrowser      = None
                    Notifications   = []
                    SearchRequestId = state.SearchRequestId + 1 },
                Cmd.batch (manageWorkerCmd path :: startupCleanupCmd path :: loadFolderTreeCmd folders :: perFolderCmds))
        | AddFolderRequested ->
            let cmd = Cmd.ofEffect (fun dispatch ->
                let opts = Avalonia.Platform.Storage.FolderPickerOpenOptions(
                    Title = "Add Folder",
                    AllowMultiple = false)
                w.StorageProvider.OpenFolderPickerAsync(opts)
                    .ContinueWith(fun (t: System.Threading.Tasks.Task<System.Collections.Generic.IReadOnlyList<Avalonia.Platform.Storage.IStorageFolder>>) ->
                        if not t.IsFaulted && not t.IsCanceled && t.Result.Count > 0 then
                            uiDispatch dispatch (FolderOpened t.Result[0].Path.LocalPath))
                |> ignore)
            Some(state, cmd)
        | FolderOpened path ->
            match state.Catalog with
            | OpenedCatalog(catalogPath) ->
                let path = IsomFolio.Core.PathUtils.normalizePath path

                let existingFolders = normalizeFolders state.Sidebar.Folders
                let alreadyTracked = existingFolders |> List.contains path
                let folders =
                    if alreadyTracked then existingFolders
                    else existingFolders @ [ path ]
                IsomFolio.Core.AppPaths.saveSession { CatalogPath = catalogPath; Folders = folders }
                if alreadyTracked then
                    Some(
                        { state with Sidebar = Sidebar.update (Sidebar.FoldersLoaded folders) state.Sidebar },
                        loadFolderTreeCmd folders)
                else
                    Some(
                        { state with
                            Sidebar      = Sidebar.update (Sidebar.FoldersLoaded folders) state.Sidebar
                            ScanProgress = Some { TotalFound = 0; Inserted = 0; FolderName = System.IO.Path.GetFileName path } },
                        Cmd.batch [ loadFolderTreeCmd folders; startScanCmd catalogPath path; createWatcherCmd path ])
            | Unloaded -> Some(state, Cmd.none)
        | _ -> None)

let private handleFileEvent (catalogPath: string) (state: State) (event: FileEvent) : State * Cmd<Msg> =
    match event with
    | Created path when isSupportedExtension (System.IO.Path.GetExtension path) ->
        let folder = System.IO.Path.GetDirectoryName path |> normalizePath
        { state with PendingFolders = state.PendingFolders |> Set.add folder }, Cmd.none
    | Deleted path ->
        state,
        Cmd.OfAsync.either
            (fun () ->
                withCatalogDb catalogPath (fun dbConn ->
                    async {
                        let folder = System.IO.Path.GetDirectoryName path
                        let! indexed = Db.getIndexedPathsInFolder dbConn folder
                        match indexed |> Map.tryFind (normalizePath path) with
                        | Some f -> do! Db.markOrphaned dbConn f.Id
                        | None   -> ()
                    }))
            ()
            (fun () -> ScanFinished 0)
            (fun ex -> AppError (ScanError ex.Message))
    | Renamed(oldPath, newPath) when isSupportedExtension (System.IO.Path.GetExtension newPath) ->
        state,
        Cmd.OfAsync.either
            (fun () ->
                withCatalogDb catalogPath (fun dbConn ->
                    async {
                        let newFile = assetFileFromInfo (System.IO.FileInfo newPath)
                        do! Db.updateFilePath dbConn (normalizePath oldPath) newFile
                        enqueueThumbnails catalogPath [ newFile ] 1
                    }))
            ()
            (fun () -> ScanFinished 0)
            (fun ex -> AppError (ScanError ex.Message))
    | Modified path when isSupportedExtension (System.IO.Path.GetExtension path) ->
        let folder = System.IO.Path.GetDirectoryName path |> normalizePath
        { state with PendingFolders = state.PendingFolders |> Set.add folder }, Cmd.none
    | SidecarChanged imagePath | SidecarRemoved imagePath ->
        let fileId = computeFileId (normalizePath imagePath)
        let isDetailFile = state.Detail.File |> Option.map (fun f -> f.Id = fileId) = Some true
        state,
        Cmd.OfAsync.either
            (fun () ->
                withCatalogDb catalogPath (fun dbConn ->
                    async {
                        let! results = Scanner.refreshMetadata [ imagePath ]
                        match results with
                        | (_, meta) :: _ ->
                            do! Db.upsertMetadata dbConn fileId meta
                            return if isDetailFile then Some meta else None
                        | [] ->
                            return None
                    }))
            ()
            (fun metaOpt ->
                match metaOpt with
                | Some meta -> DetailMsg (DetailPanel.MetadataLoaded (Some meta))
                | None -> NoOp)
            (fun ex -> AppError (ScanError ex.Message))
    | _ -> state, Cmd.none

let update (msg: Msg) (state: State) : State * Cmd<Msg> =
    match state, msg with
    | _, NewCatalogRequested
    | _, OpenCatalogRequested
    | _, CatalogOpened _
    | _, AddFolderRequested
    | _, FolderOpened _ ->
        handleCatalogMsg state msg |> Option.defaultValue (state, Cmd.none)

    | { Catalog = OpenedCatalog(catalogPath) }, SidebarMsg (Sidebar.FolderSelected _ as sbMsg) ->
        let newSidebar = Sidebar.update sbMsg state.Sidebar
        let query = { state.ActiveQuery with FolderPath = newSidebar.SelectedFolder }
        let newId = state.SearchRequestId + 1
        { state with Sidebar = newSidebar; ActiveQuery = query; SearchRequestId = newId },
        runSearch catalogPath newId query

    | { Catalog = OpenedCatalog _ }, FolderRemoveRequested path ->
        state, confirmFolderRemovalCmd path

    | _, FolderRemoveRequested _ ->
        state, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath) }, SidebarMsg (Sidebar.FolderRemoved path) ->
        let newSidebar = Sidebar.update (Sidebar.FolderRemoved path) state.Sidebar
        let remainingFolders = newSidebar.Folders
        IsomFolio.Core.AppPaths.saveSession { CatalogPath = catalogPath; Folders = remainingFolders }
        let query = { state.ActiveQuery with FolderPath = newSidebar.SelectedFolder }
        let newId = state.SearchRequestId + 1
        let newPending =
            state.PendingFolders
            |> Set.filter (fun p -> not (isWithinSubtree path p))
        { state with Sidebar = newSidebar; ActiveQuery = query; SearchRequestId = newId; PendingFolders = newPending },
        Cmd.batch [
            stopFolderWatcherCmd path
            loadFolderTreeCmd remainingFolders
            removeFolderFilesAndSearchCmd catalogPath path newId query
        ]

    | _, SidebarMsg sbMsg ->
        { state with Sidebar = Sidebar.update sbMsg state.Sidebar }, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath) }, GridMsg (GridView.TileSelected fileId) ->
        let newGrid  = GridView.update (GridView.TileSelected fileId) state.Grid
        let fileOpt =
            newGrid.Tiles
            |> List.tryFind (fun t -> t.File.Id = fileId)
            |> Option.map _.File
        let newDetail =
            fileOpt
            |> Option.map (fun f -> DetailPanel.update (DetailPanel.FileSelected f) state.Detail)
            |> Option.defaultValue state.Detail
        let loadCmds =
            fileOpt
            |> Option.map (fun f ->
                Cmd.batch [
                    Cmd.OfAsync.either
                        (fun () -> withCatalogDb catalogPath (fun dbConn -> f.Id |> Db.getTagsForFile dbConn)) ()
                        (fun tags -> DetailMsg (DetailPanel.TagsLoaded tags))
                        (fun ex  -> AppError (DbError ex.Message))
                    Cmd.OfAsync.either
                        (fun () -> withCatalogDb catalogPath Db.getAllTags) ()
                        (fun tags -> DetailMsg (DetailPanel.TagTreeMsg (TagTree.AllTagsLoaded (tags |> List.map fst))))
                        (fun ex  -> AppError (DbError ex.Message))
                    loadMetadataCmd catalogPath f.Id
                ])
            |> Option.defaultValue Cmd.none
        { state with Grid = newGrid; Detail = newDetail }, loadCmds

    | { Catalog = OpenedCatalog(catalogPath) }, GridMsg (GridView.NavigateTo _ as gMsg) ->
        let newGrid = GridView.update gMsg state.Grid
        let fileOpt =
            newGrid.SelectedId
            |> Option.bind (fun id -> newGrid.Tiles |> List.tryFind (fun t -> t.File.Id = id))
            |> Option.map _.File
        let newDetail =
            fileOpt
            |> Option.map (fun f -> DetailPanel.update (DetailPanel.FileSelected f) state.Detail)
            |> Option.defaultValue state.Detail
        let loadCmds =
            fileOpt
            |> Option.map (fun f ->
                Cmd.batch [
                    Cmd.OfAsync.either
                        (fun () -> withCatalogDb catalogPath (fun dbConn -> f.Id |> Db.getTagsForFile dbConn)) ()
                        (fun tags -> DetailMsg (DetailPanel.TagsLoaded tags))
                        (fun ex  -> AppError (DbError ex.Message))
                    Cmd.OfAsync.either
                        (fun () -> withCatalogDb catalogPath Db.getAllTags) ()
                        (fun tags -> DetailMsg (DetailPanel.TagTreeMsg (TagTree.AllTagsLoaded (tags |> List.map fst))))
                        (fun ex  -> AppError (DbError ex.Message))
                    loadMetadataCmd catalogPath f.Id
                ])
            |> Option.defaultValue Cmd.none
        { state with Grid = newGrid; Detail = newDetail }, loadCmds

    | { Catalog = OpenedCatalog(catalogPath) }, GridMsg (GridView.RemoveOrphanedFileRequested fileId) ->
        let newDetail =
            if state.Detail.File |> Option.map _.Id = Some fileId
            then DetailPanel.update DetailPanel.Closed state.Detail
            else state.Detail
        { state with Detail = newDetail },
        Cmd.OfAsync.either
            (fun () -> withCatalogDb catalogPath (fun c -> Db.deleteFile c fileId))
            ()
            (fun () -> ScanFinished 0)
            (fun ex -> AppError (DbError ex.Message))

    | _, GridMsg (GridView.OpenExternally fileId) ->
        let path = state.Grid.Tiles |> List.tryFind (fun t -> t.File.Id = fileId) |> Option.map (fun t -> t.File.Path)
        state,
        match path with
        | None -> Cmd.none
        | Some p ->
            Cmd.ofEffect (fun _ ->
                try System.Diagnostics.Process.Start(
                        System.Diagnostics.ProcessStartInfo(p, UseShellExecute = true)) |> ignore
                with _ -> ())

    | _, GridMsg (GridView.RevealInExplorer fileId) ->
        let path = state.Grid.Tiles |> List.tryFind (fun t -> t.File.Id = fileId) |> Option.map (fun t -> t.File.Path)
        state,
        match path with
        | None -> Cmd.none
        | Some p ->
            Cmd.ofEffect (fun _ ->
                try
                    if System.Runtime.InteropServices.RuntimeInformation.IsOSPlatform(System.Runtime.InteropServices.OSPlatform.OSX) then
                        System.Diagnostics.Process.Start("open", $"-R \"{p}\"") |> ignore
                    elif System.Runtime.InteropServices.RuntimeInformation.IsOSPlatform(System.Runtime.InteropServices.OSPlatform.Windows) then
                        System.Diagnostics.Process.Start("explorer", $"/select,\"{p}\"") |> ignore
                with _ -> ())

    | _, GridMsg gMsg ->
        { state with Grid = GridView.update gMsg state.Grid }, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath); Detail = { File = Some f } }, DetailMsg DetailPanel.SourceViewRequested ->
        let newDetail = DetailPanel.update DetailPanel.SourceViewRequested state.Detail
        { state with Detail = newDetail }, loadSourceViewCmd f.Path f.Id

    | { Catalog = OpenedCatalog(catalogPath); Detail = { File = Some f } }, DetailMsg (DetailPanel.TagTreeMsg tMsg) when TagTree.isMutating tMsg ->
        let newDetail = DetailPanel.update (DetailPanel.TagTreeMsg tMsg) state.Detail
        let newTags = TagTree.flattenTree newDetail.TagTree.Roots
        let cmds =
            Cmd.batch [
                Cmd.OfAsync.either
                    (fun () -> withCatalogDb catalogPath (fun c -> Db.upsertTags c f.Id newTags))
                    ()
                    (fun () -> TagsUpdated (f.Id, newTags))
                    (fun ex -> AppError (DbError ex.Message))
                Cmd.OfAsync.either
                    (fun () -> withCatalogDb catalogPath Db.getAllTags) ()
                    (fun tags -> DetailMsg (DetailPanel.TagTreeMsg (TagTree.AllTagsLoaded (tags |> List.map fst))))
                    (fun ex  -> AppError (DbError ex.Message))
            ]
        { state with Detail = newDetail }, cmds

    | { Catalog = OpenedCatalog(catalogPath) }, DetailMsg DetailPanel.TagBrowserRequested ->
        { state with TagBrowser = Some (TagBrowser.init ()) },
        Cmd.OfAsync.either
            (fun () -> withCatalogDb catalogPath Db.getAllTags)
            ()
            (fun tags -> TagBrowserMsg (TagBrowser.TagsLoaded tags))
            (fun ex -> AppError (DbError ex.Message))

    | { Catalog = OpenedCatalog(_) }, DetailMsg dMsg ->
        { state with Detail = DetailPanel.update dMsg state.Detail }, Cmd.none

    | _, DetailMsg dMsg ->
        { state with Detail = DetailPanel.update dMsg state.Detail }, Cmd.none

    | _, TagBrowserMsg TagBrowser.Closed ->
        { state with TagBrowser = None }, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath); TagBrowser = Some browser }, TagBrowserMsg TagBrowser.RenameSubmitted ->
        match browser.RenameInput with
        | None -> state, Cmd.none
        | Some (oldTag, newText) ->
            let newTag = newText.Trim()
            if newTag = "" || newTag = oldTag then
                { state with TagBrowser = Some (TagBrowser.update TagBrowser.RenameCancelled browser) }, Cmd.none
            else
                state,
                Cmd.OfAsync.either
                    (fun () -> async {
                        let! _ = withCatalogDb catalogPath (fun c -> Db.renamePrefixedTags c oldTag newTag)
                        return! withCatalogDb catalogPath Db.getAllTags
                    })
                    ()
                    (fun tags -> TagBrowserMsg (TagBrowser.MutationCompleted tags))
                    (fun ex -> AppError (DbError ex.Message))

    | { Catalog = OpenedCatalog(catalogPath); TagBrowser = Some browser }, TagBrowserMsg TagBrowser.DeleteConfirmed ->
        match browser.PendingDelete with
        | None -> state, Cmd.none
        | Some tag ->
            state,
            Cmd.OfAsync.either
                (fun () -> async {
                    let! _ = withCatalogDb catalogPath (fun c -> Db.deleteTagWithDescendants c tag)
                    return! withCatalogDb catalogPath Db.getAllTags
                })
                ()
                (fun tags -> TagBrowserMsg (TagBrowser.MutationCompleted tags))
                (fun ex -> AppError (DbError ex.Message))

    | { Catalog = OpenedCatalog(catalogPath); TagBrowser = Some _ }, TagBrowserMsg (TagBrowser.MutationCompleted tags) ->
        let newBrowser = state.TagBrowser |> Option.map (TagBrowser.update (TagBrowser.MutationCompleted tags))
        let newDetail = DetailPanel.update (DetailPanel.TagTreeMsg (TagTree.AllTagsLoaded (tags |> List.map fst))) state.Detail
        let reloadTagsCmd =
            match state.Detail.File with
            | None -> Cmd.none
            | Some f ->
                Cmd.OfAsync.either
                    (fun () -> withCatalogDb catalogPath (fun c -> Db.getTagsForFile c f.Id))
                    ()
                    (fun t -> DetailMsg (DetailPanel.TagsLoaded t))
                    (fun ex -> AppError (DbError ex.Message))
        { state with TagBrowser = newBrowser; Detail = newDetail }, reloadTagsCmd

    | _, TagBrowserMsg bMsg ->
        match state.TagBrowser with
        | None -> state, Cmd.none
        | Some browser ->
            { state with TagBrowser = Some (TagBrowser.update bMsg browser) }, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath) }, SearchBarMsg (SearchBar.QuerySubmitted txt) ->
        let query = { state.ActiveQuery with Text = if txt.Trim() = "" then None else Some txt }
        let newId = state.SearchRequestId + 1
        { state with ActiveQuery = query; SearchRequestId = newId }, runSearch catalogPath newId query

    | _, SearchBarMsg sbMsg ->
        { state with SearchBar = SearchBar.update sbMsg state.SearchBar }, Cmd.none

    | _, ScanProgressUpdated progress ->
        { state with ScanProgress = Some progress }, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath) }, ScanBatchCompleted files when not (isFilterActive state.ActiveQuery) ->
        let newGrid =
            state.Grid.Tiles
            |> List.map (fun t -> t.File)
            |> fun existing -> existing @ files
            |> List.distinctBy (fun f -> f.Id)
            |> GridView.TilesLoaded
            |> GridView.update <| state.Grid
            |> primeGridThumbnails catalogPath 1
        { state with Grid = newGrid }, Cmd.none

    | _, ScanBatchCompleted _ ->
        state, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath) }, ScanFinished _ ->
        let newId = state.SearchRequestId + 1
        { state with ScanProgress = None; SearchRequestId = newId },
        Cmd.batch [ runSearch catalogPath newId state.ActiveQuery; countOrphansCmd catalogPath ]

    | _, ScanFinished _ ->
        { state with ScanProgress = None }, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath) }, SearchCompleted(id, files) when id = state.SearchRequestId ->
        let newGrid =
            state.Grid
            |> GridView.update (GridView.TilesLoaded files)
            |> primeGridThumbnails catalogPath 1
        
        let newDetail =
            match newGrid.SelectedId with
            | Some sid ->
                newGrid.Tiles
                |> List.tryFind (fun t -> t.File.Id = sid)
                |> Option.map (fun t -> DetailPanel.update (DetailPanel.FileSelected t.File) state.Detail)
                |> Option.defaultValue state.Detail
            | None -> state.Detail

        { state with Grid = newGrid; Detail = newDetail }, Cmd.none

    | _, SearchCompleted _ ->
        state, Cmd.none

    | _, TagsUpdated _ ->
        state, Cmd.none

    | _, AppError err ->
        let msg = formatError err
        let t = System.DateTime.UtcNow
        let dismissCmd =
            Cmd.OfAsync.either
                (fun () -> async {
                    do! Async.Sleep 5000
                    return t
                })
                ()
                DismissNotification
                (fun _ -> NoOp)
        { state with Notifications = (msg, t) :: state.Notifications |> List.truncate 5 }, dismissCmd

    | _, OrphanCountLoaded n ->
        { state with OrphanCount = n }, Cmd.none

    | _, DismissNotification t ->
        { state with Notifications = state.Notifications |> List.filter (fun (_, ts) -> ts <> t) }, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath) }, ResyncFolderRequested path ->
        state, resyncFolderCmd catalogPath path

    | { Catalog = OpenedCatalog(catalogPath) }, FolderResynced path ->
        let newPending =
            state.PendingFolders
            |> Set.filter (fun p -> not (isWithinSubtree path p))
        let newId = state.SearchRequestId + 1
        { state with PendingFolders = newPending; SearchRequestId = newId },
        Cmd.batch [ runSearch catalogPath newId state.ActiveQuery; countOrphansCmd catalogPath ]

    | { Catalog = OpenedCatalog(catalogPath) }, FileEventReceived event ->
        handleFileEvent catalogPath state event

    | _, FileEventReceived _ | _, ResyncFolderRequested _ | _, FolderResynced _ | _, NoOp -> state, Cmd.none

let private welcomeView (dispatch: Msg -> unit) =
    DockPanel.create [
        DockPanel.children [
            StackPanel.create [
                StackPanel.verticalAlignment VerticalAlignment.Center
                StackPanel.horizontalAlignment HorizontalAlignment.Center
                StackPanel.spacing 16.0
                StackPanel.children [
                    TextBlock.create [
                        TextBlock.text "IsomFolio"
                        TextBlock.fontSize 32.0
                        TextBlock.fontWeight FontWeight.Light
                        TextBlock.foreground Brushes.White
                        TextBlock.horizontalAlignment HorizontalAlignment.Center
                    ]
                    TextBlock.create [
                        TextBlock.text "Your files stay on disk. Tags travel with them."
                        TextBlock.fontSize Theme.FontSize.lg
                        TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                        TextBlock.horizontalAlignment HorizontalAlignment.Center
                    ]
                    StackPanel.create [
                        StackPanel.orientation Orientation.Horizontal
                        StackPanel.horizontalAlignment HorizontalAlignment.Center
                        StackPanel.spacing 8.0
                        StackPanel.children [
                            Button.create [
                                Button.content "New Catalog…"
                                Button.fontSize Theme.FontSize.xl
                                Button.padding (Avalonia.Thickness(24.0, 10.0))
                                Button.onClick (fun _ -> dispatch NewCatalogRequested)
                            ]
                            Button.create [
                                Button.content "Open Catalog…"
                                Button.fontSize Theme.FontSize.xl
                                Button.padding (Avalonia.Thickness(24.0, 10.0))
                                Button.onClick (fun _ -> dispatch OpenCatalogRequested)
                            ]
                        ]
                    ]
                ]
            ]
        ]
    ] :> Avalonia.FuncUI.Types.IView

let private mainPanel (state: State) (dispatch: Msg -> unit) =
    DockPanel.create [
        DockPanel.background (SolidColorBrush(Theme.mainBg))
        DockPanel.children [
            // Fixed index 0: search bar
            DockPanel.create [
                DockPanel.dock Dock.Top
                DockPanel.background (SolidColorBrush(Theme.panelBg))
                DockPanel.height 40.0
                DockPanel.children [
                    SearchBar.view state.SearchBar (SearchBarMsg >> dispatch)
                ]
            ]
            // Fixed index 1: status area — orphan banner, notifications, scan progress
            // Wrapped in a StackPanel so its variable inner content never shifts outer DockPanel indices
            StackPanel.create [
                StackPanel.dock Dock.Top
                StackPanel.children [
                    Border.create [
                        Border.isVisible false
                    ]
                    for (msg, t) in state.Notifications do
                        Border.create [
                            Border.background (SolidColorBrush(Theme.errorBg))
                            Border.padding (Avalonia.Thickness(8.0, 4.0))
                            Border.child (
                                DockPanel.create [
                                    DockPanel.children [
                                        Button.create [
                                            Button.dock Dock.Right
                                            Button.content "✕"
                                            Button.fontSize 10.0
                                            Button.padding (Avalonia.Thickness(4.0, 0.0))
                                            Button.background Brushes.Transparent
                                            Button.foreground Brushes.White
                                            Button.onClick (fun _ -> dispatch (DismissNotification t))
                                        ]
                                        TextBlock.create [
                                            TextBlock.text msg
                                            TextBlock.foreground Brushes.White
                                            TextBlock.fontSize Theme.FontSize.sm
                                            TextBlock.verticalAlignment VerticalAlignment.Center
                                        ]
                                    ]
                                ])
                        ] :> Avalonia.FuncUI.Types.IView
                    Border.create [
                        Border.isVisible state.ScanProgress.IsSome
                        Border.background (SolidColorBrush(Theme.scanBarBg))
                        Border.height 28.0
                        Border.child (
                            match state.ScanProgress with
                            | None -> TextBlock.create [] :> Avalonia.FuncUI.Types.IView
                            | Some p ->
                                DockPanel.create [
                                    DockPanel.children [
                                        TextBlock.create [
                                            TextBlock.dock Dock.Right
                                            TextBlock.text $"{p.Inserted} indexed"
                                            TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                            TextBlock.fontSize Theme.FontSize.xs
                                            TextBlock.verticalAlignment VerticalAlignment.Center
                                            TextBlock.margin (Avalonia.Thickness(0.0, 0.0, 8.0, 0.0))
                                        ]
                                        ProgressBar.create [
                                            ProgressBar.dock Dock.Right
                                            ProgressBar.isIndeterminate true
                                            ProgressBar.width 80.0
                                            ProgressBar.height 4.0
                                            ProgressBar.verticalAlignment VerticalAlignment.Center
                                            ProgressBar.margin (Avalonia.Thickness(0.0, 0.0, 8.0, 0.0))
                                        ]
                                        TextBlock.create [
                                            TextBlock.text $"Scanning {p.FolderName}…"
                                            TextBlock.foreground Brushes.White
                                            TextBlock.fontSize Theme.FontSize.xs
                                            TextBlock.verticalAlignment VerticalAlignment.Center
                                            TextBlock.margin (Avalonia.Thickness(8.0, 0.0))
                                        ]
                                    ]
                                ] :> Avalonia.FuncUI.Types.IView)
                    ]
                ]
            ]
            // Fixed index 2: sidebar
            Border.create [
                Border.dock Dock.Left
                Border.width 220.0
                Border.isVisible (not state.IsFirstRun)
                Border.child (
                    let catalogName =
                        match state.Catalog with
                        | OpenedCatalog p -> Some (System.IO.Path.GetFileName p)
                        | Unloaded -> None
                    Sidebar.view state.Sidebar (SidebarMsg >> dispatch) state.PendingFolders (fun () -> dispatch AddFolderRequested) (fun path -> dispatch (FolderRemoveRequested path)) (fun path -> dispatch (ResyncFolderRequested path)) catalogName (fun () -> dispatch NewCatalogRequested) (fun () -> dispatch OpenCatalogRequested))
            ]
            // Fixed index 3: detail panel
            Border.create [
                Border.dock Dock.Right
                Border.isVisible (state.Detail.IsVisible && not state.IsFirstRun)
                Border.child (DetailPanel.view state.Detail (DetailMsg >> dispatch))
            ]
            // Fixed index 4: center — welcome and grid always present, toggled by isVisible
            Grid.create [
                Grid.children [
                    Border.create [
                        Border.isVisible state.IsFirstRun
                        Border.child (welcomeView dispatch)
                    ] :> Avalonia.FuncUI.Types.IView
                    Border.create [
                        Border.isVisible (not state.IsFirstRun)
                        Border.child (GridView.view state.Grid (GridMsg >> dispatch))
                    ] :> Avalonia.FuncUI.Types.IView
                ]
            ]
        ]
    ] :> Avalonia.FuncUI.Types.IView

let view (state: State) (dispatch: Msg -> unit) =
    Grid.create [
        Grid.children [
            yield mainPanel state dispatch
            match state.TagBrowser with
            | Some browser ->
                yield Grid.create [
                    Grid.background (SolidColorBrush(Avalonia.Media.Color.FromArgb(200uy, 0uy, 0uy, 0uy)))
                    Grid.children [
                        TagBrowser.view browser (TagBrowserMsg >> dispatch)
                    ]
                ] :> Avalonia.FuncUI.Types.IView
            | None -> ()
        ]
    ] :> Avalonia.FuncUI.Types.IView
