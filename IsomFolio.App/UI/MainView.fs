module IsomFolio.UI.MainView

open Elmish
open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
open Avalonia.Threading
open IsomFolio.Models
open IsomFolio.FileIndex
open IsomFolio.Indexing
open IsomFolio.Storage
open IsomFolio.Search
open Microsoft.Data.Sqlite

let mutable private thumbnailWorker: MailboxProcessor<Thumbnail.ThumbnailMsg> option = None
let mutable private activeWatchers: System.IO.FileSystemWatcher list = []

type CatalogState =
    | Unloaded
    | OpenedCatalog of catalogPath: string * dbConn: SqliteConnection

type State = {
    Sidebar      : Sidebar.State
    Grid         : GridView.State
    Detail       : DetailPanel.State
    SearchBar    : SearchBar.State
    ScanProgress : ScanProgress option
    ActiveQuery  : SearchQuery
    Errors       : AppError list
    IsFirstRun   : bool
    Catalog      : CatalogState
    Window       : Window
}

type Msg =
    | SidebarMsg          of Sidebar.Msg
    | GridMsg             of GridView.Msg
    | DetailMsg           of DetailPanel.Msg
    | SearchBarMsg        of SearchBar.Msg
    | FolderOpened        of folderPath: string
    | ScanBatchCompleted  of AssetFile list
    | ScanFinished        of totalCount: int
    | SearchCompleted     of AssetFile list
    | ThumbnailReady      of FileId * string
    | ThumbnailFailed     of FileId * string
    | FileEventReceived   of FileEvent
    | TagsUpdated         of FileId * string list
    | TagsSaved           of FileId * string list
    | AppError            of AppError
    | NewCatalogRequested
    | OpenCatalogRequested
    | CatalogOpened       of catalogPath: string * dbConn: SqliteConnection * folders: string list
    | NoOp

let private defaultQuery = {
    Text = None; Tags = []; Extensions = []
    DateRange = None; SortBy = Date; SortAsc = false
}

let init (w: Window) () : State * Cmd<Msg> =
    let state = {
        Sidebar      = Sidebar.init ()
        Grid         = GridView.init ()
        Detail       = DetailPanel.init ()
        SearchBar    = SearchBar.init ()
        ScanProgress = None
        ActiveQuery  = defaultQuery
        Errors       = []
        IsFirstRun   = true
        Catalog      = Unloaded
        Window       = w
    }
    let initCmd =
        match IsomFolio.AppPaths.readLastSession() with
        | None -> Cmd.none
        | Some session ->
            Cmd.OfAsync.either
                (fun () -> async {
                    let! conn = Db.openDatabase (IsomFolio.AppPaths.dbPath session.CatalogPath)
                    return session.CatalogPath, conn, session.Folders
                })
                ()
                CatalogOpened
                (fun ex -> AppError (DbError ex.Message))
    state, initCmd

let private runSearch (c: SqliteConnection) (query: SearchQuery) : Cmd<Msg> =
    Cmd.OfAsync.either
        (fun () -> query |> QueryEngine.executeSearch c)
        ()
        SearchCompleted
        (fun ex -> AppError (DbError ex.Message))

let private startThumbnailWorkerCmd (catalogPath: string) : Cmd<Msg> =
    Cmd.ofEffect (fun dispatch ->
        let worker =
            Thumbnail.createWorkerPool catalogPath 4
                (fun fileId path ->
                    Dispatcher.UIThread.Post(fun () -> dispatch (ThumbnailReady(fileId, path))))
                (fun fileId msg ->
                    Dispatcher.UIThread.Post(fun () -> dispatch (ThumbnailFailed(fileId, msg))))
        thumbnailWorker <- Some worker)

let private startScanCmd (catalogPath: string) (dbConn: SqliteConnection) (folderPath: string) : Cmd<Msg> =
    Cmd.ofEffect (fun dispatch ->
        Async.Start(async {
            let! result =
                Scanner.scanFolder folderPath
                    (fun batch -> async {
                        let! _ = Db.upsertFiles dbConn batch
                        Dispatcher.UIThread.Post(fun () -> dispatch (ScanBatchCompleted batch))
                    })
                    ignore
            Dispatcher.UIThread.Post(fun () -> dispatch (ScanFinished result.TotalCount))
        }))

let private createWatcherCmd (folderPath: string) : Cmd<Msg> =
    Cmd.ofEffect (fun dispatch ->
        let w = Watcher.createWatcher folderPath (fun event ->
            Dispatcher.UIThread.Post(fun () -> dispatch (FileEventReceived event)))
        activeWatchers <- w :: activeWatchers)

let private reconcileFolderCmd (dbConn: SqliteConnection) (folderPath: string) : Cmd<Msg> =
    Cmd.OfAsync.either
        (fun () -> async {
            let! newPaths, orphanedIds = Scanner.reconcileFolder dbConn folderPath
            for oid in orphanedIds do
                do! Db.markOrphaned dbConn oid
            let newFiles =
                newPaths |> List.choose (fun p ->
                    try Some(assetFileFromInfo (System.IO.FileInfo p))
                    with _ -> None)
            if not newFiles.IsEmpty then
                let! _ = Db.upsertFiles dbConn newFiles
                ()
        })
        ()
        (fun () -> ScanFinished 0)
        (fun ex -> AppError (ScanError ex.Message))

let private enqueueThumbnails (catalogPath: string) (files: AssetFile list) (priority: int) =
    match thumbnailWorker with
    | Some w ->
        for f in files do
            if not (Thumbnail.isCacheValid catalogPath f.Id) then
                w.Post(Thumbnail.Enqueue { FileId = f.Id; FilePath = f.Path; Priority = priority })
    | None -> ()

let update (msg: Msg) (state: State) : State * Cmd<Msg> =
    match state, msg with
    | { Window = w }, NewCatalogRequested ->
        let cmd =
            Cmd.OfAsync.either
                (fun () -> async {
                    let opts = Avalonia.Platform.Storage.FilePickerSaveOptions(
                        Title = "Create New Catalog",
                        SuggestedFileName = "my-library")
                    let! file = w.StorageProvider.SaveFilePickerAsync(opts) |> Async.AwaitTask
                    if isNull file then return NoOp
                    else
                        let rawPath = file.Path.LocalPath
                        let parentDir = System.IO.Path.GetDirectoryName(rawPath)
                        let baseName  = System.IO.Path.GetFileNameWithoutExtension(rawPath)
                        let catalogPath = IsomFolio.AppPaths.createCatalog parentDir baseName
                        let! conn = Db.openDatabase (IsomFolio.AppPaths.dbPath catalogPath)
                        return CatalogOpened (catalogPath, conn, []) })
                ()
                id
                (fun ex -> AppError (ScanError ex.Message))
        state, cmd

    | { Window = w }, OpenCatalogRequested ->
        let cmd =
            Cmd.OfAsync.either
                (fun () -> async {
                    let opts = Avalonia.Platform.Storage.FolderPickerOpenOptions(Title = "Open Catalog", AllowMultiple = false)
                    let! picked = w.StorageProvider.OpenFolderPickerAsync(opts) |> Async.AwaitTask
                    if picked.Count = 0 then return NoOp
                    else
                        let catalogPath = picked[0].Path.LocalPath
                        let! conn = Db.openDatabase (IsomFolio.AppPaths.dbPath catalogPath)
                        return CatalogOpened (catalogPath, conn, []) })
                ()
                id
                (fun ex -> AppError (DbError ex.Message))
        state, cmd

    | _, CatalogOpened (path, conn, folders) ->
        IsomFolio.AppPaths.saveSession { CatalogPath = path; Folders = folders }
        let workerCmd = startThumbnailWorkerCmd path
        let perFolderCmds =
            folders |> List.collect (fun f -> [ reconcileFolderCmd conn f; createWatcherCmd f ])
        let scanProgress =
            if folders.IsEmpty then None
            else Some { TotalFound = 0; Inserted = 0; FolderName = "Restoring…" }
        { state with
            Catalog = OpenedCatalog(path, conn)
            IsFirstRun   = false
            Sidebar      = Sidebar.update (Sidebar.FoldersLoaded folders) state.Sidebar
            ScanProgress = scanProgress },
        Cmd.batch (workerCmd :: perFolderCmds)

    | { Window = w }, SidebarMsg Sidebar.AddFolderRequested ->
        let cmd =
            Cmd.OfAsync.either
                (fun () -> async {
                    let opts = Avalonia.Platform.Storage.FolderPickerOpenOptions(Title = "Add Folder", AllowMultiple = false)
                    let! picked = w.StorageProvider.OpenFolderPickerAsync(opts) |> Async.AwaitTask
                    if picked.Count > 0 then return FolderOpened picked[0].Path.LocalPath
                    else return NoOp })
                ()
                id
                (fun ex -> AppError (ScanError ex.Message))
        state, cmd

    | _, SidebarMsg sbMsg ->
        { state with Sidebar = Sidebar.update sbMsg state.Sidebar }, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath, dbConn) }, FolderOpened path ->
        let folders = state.Sidebar.Folders @ [ path ]
        IsomFolio.AppPaths.saveSession { CatalogPath = catalogPath; Folders = folders }
        { state with
            Sidebar      = Sidebar.update (Sidebar.FoldersLoaded folders) state.Sidebar
            ScanProgress = Some { TotalFound = 0; Inserted = 0; FolderName = System.IO.Path.GetFileName path } },
        Cmd.batch [ startScanCmd catalogPath dbConn path; createWatcherCmd path ]

    | _, FolderOpened _ -> state, Cmd.none

    | { Catalog = OpenedCatalog(_, dbConn) }, GridMsg (GridView.TileSelected fileId) ->
        let newGrid  = GridView.update (GridView.TileSelected fileId) state.Grid
        let fileOpt =
            state.Grid.Tiles
            |> List.tryFind (fun t -> t.File.Id = fileId)
            |> Option.map _.File
        let newDetail =
            fileOpt
            |> Option.map (fun f -> DetailPanel.update (DetailPanel.FileSelected f) state.Detail)
            |> Option.defaultValue state.Detail
        thumbnailWorker |> Option.iter _.Post(Thumbnail.SetPriority(fileId, 0))
        let loadTagsCmd =
            fileOpt
            |> Option.map (fun f ->
                Cmd.OfAsync.either
                    (fun () -> f.Id |> Db.getTagsForFile dbConn) ()
                    (fun tags -> DetailMsg (DetailPanel.TagsLoaded tags))
                    (fun ex  -> AppError (DbError ex.Message)))
            |> Option.defaultValue Cmd.none
        { state with Grid = newGrid; Detail = newDetail }, loadTagsCmd

    | _, GridMsg gMsg ->
        { state with Grid = GridView.update gMsg state.Grid }, Cmd.none

    | { Catalog = OpenedCatalog(_, dbConn); Detail = { File = Some f } }, DetailMsg DetailPanel.AddTagRequested
        when state.Detail.TagInput.Trim() <> "" ->
        let tag = state.Detail.TagInput.Trim()
        let cmd =
            Cmd.OfAsync.either
                (fun () -> async {
                    let! result = IsomFolio.Tagging.Tagging.addTag f.Path tag
                    match result with
                    | Ok newTags ->
                        do! newTags |> Db.upsertTags dbConn f.Id
                        do! newTags |> FTS.updateFileIndexTags dbConn f.Id
                        return TagsSaved(f.Id, newTags)
                    | Error e -> return AppError (XmpWriteError(f.Path, e))
                })
                ()
                id
                (fun ex -> AppError (XmpWriteError(f.Path, ex.Message)))
        { state with Detail = DetailPanel.update (DetailPanel.TagInputChanged "") state.Detail }, cmd

    | { Catalog = OpenedCatalog(_, dbConn); Detail = { File = Some f } }, DetailMsg (DetailPanel.RemoveTagRequested tag) ->
        let cmd =
            Cmd.OfAsync.either
                (fun () -> async {
                    let! result = IsomFolio.Tagging.Tagging.removeTag f.Path tag
                    match result with
                    | Ok newTags ->
                        do! newTags |> Db.upsertTags dbConn f.Id
                        do! newTags |> FTS.updateFileIndexTags dbConn f.Id
                        return TagsSaved(f.Id, newTags)
                    | Error e -> return AppError (XmpWriteError(f.Path, e))
                })
                ()
                id
                (fun ex -> AppError (XmpWriteError(f.Path, ex.Message)))
        state, cmd

    | { Detail = { File = Some f } }, DetailMsg DetailPanel.OpenExternally ->
        try System.Diagnostics.Process.Start(
                System.Diagnostics.ProcessStartInfo(f.Path, UseShellExecute = true)) |> ignore
        with _ -> ()
        state, Cmd.none

    | { Detail = { File = Some f } }, DetailMsg DetailPanel.RevealInExplorer ->
        try
            if System.Runtime.InteropServices.RuntimeInformation.IsOSPlatform(System.Runtime.InteropServices.OSPlatform.OSX) then
                System.Diagnostics.Process.Start("open", $"-R \"{f.Path}\"") |> ignore
            elif System.Runtime.InteropServices.RuntimeInformation.IsOSPlatform(System.Runtime.InteropServices.OSPlatform.Windows) then
                System.Diagnostics.Process.Start("explorer", $"/select,\"{f.Path}\"") |> ignore
        with _ -> ()
        state, Cmd.none

    | _, DetailMsg dMsg ->
        { state with Detail = DetailPanel.update dMsg state.Detail }, Cmd.none

    | { Catalog = OpenedCatalog(_, dbConn) }, SearchBarMsg (SearchBar.QuerySubmitted txt) ->
        let query = { state.ActiveQuery with Text = if txt.Trim() = "" then None else Some txt }
        { state with ActiveQuery = query }, query |> runSearch dbConn

    | _, SearchBarMsg sbMsg ->
        { state with SearchBar = SearchBar.update sbMsg state.SearchBar }, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath, _) }, ScanBatchCompleted files ->
        let current = state.ScanProgress |> Option.defaultValue { TotalFound = 0; Inserted = 0; FolderName = "" }
        let progress = { current with TotalFound = current.TotalFound + files.Length; Inserted = current.Inserted + files.Length }
        let newGrid = GridView.update (GridView.TilesLoaded (state.Grid.Tiles |> List.map (fun t -> t.File) |> (@) files)) state.Grid
        enqueueThumbnails catalogPath files 1
        { state with ScanProgress = Some progress; Grid = newGrid }, Cmd.none

    | _, ScanBatchCompleted files ->
        let current = state.ScanProgress |> Option.defaultValue { TotalFound = 0; Inserted = 0; FolderName = "" }
        let progress = { current with TotalFound = current.TotalFound + files.Length; Inserted = current.Inserted + files.Length }
        let newGrid = GridView.update (GridView.TilesLoaded (state.Grid.Tiles |> List.map (fun t -> t.File) |> (@) files)) state.Grid
        { state with ScanProgress = Some progress; Grid = newGrid }, Cmd.none

    | { Catalog = OpenedCatalog(_, dbConn) }, ScanFinished _ ->
        { state with ScanProgress = None }, state.ActiveQuery |> runSearch dbConn

    | _, ScanFinished _ ->
        { state with ScanProgress = None }, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath, _) }, SearchCompleted files ->
        enqueueThumbnails catalogPath files 1
        { state with Grid = GridView.update (GridView.TilesLoaded files) state.Grid }, Cmd.none

    | _, SearchCompleted files ->
        { state with Grid = GridView.update (GridView.TilesLoaded files) state.Grid }, Cmd.none

    | _, ThumbnailReady(fileId, path) ->
        { state with Grid = GridView.update (GridView.ThumbnailUpdated(fileId, Ready path)) state.Grid }, Cmd.none

    | _, ThumbnailFailed(fileId, _) ->
        { state with Grid = GridView.update (GridView.ThumbnailUpdated(fileId, Failed 2)) state.Grid }, Cmd.none

    | _, TagsUpdated(fileId, tags) | _, TagsSaved(fileId, tags) ->
        let newDetail =
            if state.Detail.File |> Option.map (fun f -> f.Id) = Some fileId
            then DetailPanel.update (DetailPanel.TagsLoaded tags) state.Detail
            else state.Detail
        { state with Detail = newDetail }, Cmd.none

    | _, AppError err ->
        { state with Errors = err :: state.Errors |> List.truncate 5 }, Cmd.none

    | { Catalog = OpenedCatalog(catalogPath, dbConn) }, FileEventReceived (Created path)
        when isSupportedExtension (System.IO.Path.GetExtension path) ->
        state,
        Cmd.OfAsync.either
            (fun () -> async {
                let f = assetFileFromInfo (System.IO.FileInfo path)
                let! _ = Db.upsertFiles dbConn [ f ]
                enqueueThumbnails catalogPath [ f ] 1
            })
            ()
            (fun () -> ScanFinished 0)
            (fun ex -> AppError (ScanError ex.Message))

    | { Catalog = OpenedCatalog(_, dbConn) }, FileEventReceived (Deleted path) ->
        state,
        Cmd.OfAsync.either
            (fun () -> async {
                let folder = System.IO.Path.GetDirectoryName path
                let! indexed = Db.getIndexedPathsInFolder dbConn folder
                match indexed |> Map.tryFind path with
                | Some f -> do! Db.markOrphaned dbConn f.Id
                | None   -> ()
            })
            ()
            (fun () -> ScanFinished 0)
            (fun ex -> AppError (ScanError ex.Message))

    | { Catalog = OpenedCatalog(catalogPath, dbConn) }, FileEventReceived (Renamed(oldPath, newPath))
        when isSupportedExtension (System.IO.Path.GetExtension newPath) ->
        state,
        Cmd.OfAsync.either
            (fun () -> async {
                let newFile = assetFileFromInfo (System.IO.FileInfo newPath)
                do! Db.updateFilePath dbConn oldPath newFile
                enqueueThumbnails catalogPath [ newFile ] 1
            })
            ()
            (fun () -> ScanFinished 0)
            (fun ex -> AppError (ScanError ex.Message))

    | { Catalog = OpenedCatalog(catalogPath, dbConn) }, FileEventReceived (Modified path)
        when isSupportedExtension (System.IO.Path.GetExtension path) ->
        state,
        Cmd.OfAsync.either
            (fun () -> async {
                let f = assetFileFromInfo (System.IO.FileInfo path)
                let! _ = Db.upsertFiles dbConn [ f ]
                enqueueThumbnails catalogPath [ f ] 0
            })
            ()
            (fun () -> ScanFinished 0)
            (fun ex -> AppError (ScanError ex.Message))

    | _, FileEventReceived _ | _, NoOp -> state, Cmd.none

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
                        TextBlock.fontSize 14.0
                        TextBlock.foreground (SolidColorBrush(Color.Parse("#888888")))
                        TextBlock.horizontalAlignment HorizontalAlignment.Center
                    ]
                    StackPanel.create [
                        StackPanel.orientation Orientation.Horizontal
                        StackPanel.horizontalAlignment HorizontalAlignment.Center
                        StackPanel.spacing 8.0
                        StackPanel.children [
                            Button.create [
                                Button.content "New Catalog…"
                                Button.fontSize 16.0
                                Button.padding (Avalonia.Thickness(24.0, 10.0))
                                Button.onClick (fun _ -> dispatch NewCatalogRequested)
                            ]
                            Button.create [
                                Button.content "Open Catalog…"
                                Button.fontSize 16.0
                                Button.padding (Avalonia.Thickness(24.0, 10.0))
                                Button.onClick (fun _ -> dispatch OpenCatalogRequested)
                            ]
                        ]
                    ]
                ]
            ]
        ]
    ] :> Avalonia.FuncUI.Types.IView

let private progressView (progress: ScanProgress) =
    StackPanel.create [
        StackPanel.verticalAlignment VerticalAlignment.Center
        StackPanel.horizontalAlignment HorizontalAlignment.Center
        StackPanel.spacing 8.0
        StackPanel.children [
            TextBlock.create [
                TextBlock.text $"Scanning {progress.FolderName}…"
                TextBlock.foreground Brushes.White
                TextBlock.horizontalAlignment HorizontalAlignment.Center
            ]
            ProgressBar.create [
                ProgressBar.isIndeterminate true
                ProgressBar.width 300.0
            ]
            TextBlock.create [
                TextBlock.text $"{progress.Inserted} files indexed"
                TextBlock.foreground (SolidColorBrush(Color.Parse("#888888")))
                TextBlock.horizontalAlignment HorizontalAlignment.Center
            ]
        ]
    ] :> Avalonia.FuncUI.Types.IView

let view (state: State) (dispatch: Msg -> unit) =
    DockPanel.create [
        DockPanel.background (SolidColorBrush(Color.Parse("#252526")))
        DockPanel.children [
            DockPanel.create [
                DockPanel.dock Dock.Top
                DockPanel.background (SolidColorBrush(Color.Parse("#1E1E1E")))
                DockPanel.height 40.0
                DockPanel.children [
                    SearchBar.view state.SearchBar (SearchBarMsg >> dispatch)
                ]
            ]
            if not state.Errors.IsEmpty then
                Border.create [
                    Border.dock Dock.Top
                    Border.background (SolidColorBrush(Color.Parse("#C42B1C")))
                    Border.padding (Avalonia.Thickness(8.0, 4.0))
                    Border.child (
                        TextBlock.create [
                            TextBlock.text (state.Errors |> List.head |> sprintf "%A")
                            TextBlock.foreground Brushes.White
                            TextBlock.fontSize 12.0
                        ])
                ]
            Sidebar.view state.Sidebar (SidebarMsg >> dispatch)
            DetailPanel.view state.Detail (DetailMsg >> dispatch)
            match state.IsFirstRun with
            | true  -> welcomeView dispatch
            | false ->
                match state.ScanProgress with
                | Some p -> progressView p
                | None   -> GridView.view state.Grid (GridMsg >> dispatch)
        ]
    ] :> Avalonia.FuncUI.Types.IView
