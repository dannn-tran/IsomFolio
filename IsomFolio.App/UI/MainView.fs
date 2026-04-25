module IsomFolio.UI.MainView

open Elmish
open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
open IsomFolio.Models
open IsomFolio.Storage
open IsomFolio.Search

/// Set by MainWindow constructor — required for StorageProvider folder picker
let mutable window: Avalonia.Controls.Window option = None

type State = {
    Sidebar     : Sidebar.State
    Grid        : GridView.State
    Detail      : DetailPanel.State
    SearchBar   : SearchBar.State
    ScanProgress: ScanProgress option
    ActiveQuery : SearchQuery
    Errors      : AppError list
    IsFirstRun  : bool
    CatalogPath : string option
}

type Msg =
    | SidebarMsg     of Sidebar.Msg
    | GridMsg        of GridView.Msg
    | DetailMsg      of DetailPanel.Msg
    | SearchBarMsg   of SearchBar.Msg
    | FolderOpened   of folderPath: string
    | ScanBatchCompleted of AssetFile list
    | ScanFinished   of totalCount: int
    | SearchCompleted of AssetFile list
    | ThumbnailReady  of FileId * string
    | ThumbnailFailed of FileId * string
    | FileEventReceived of FileEvent
    | TagsUpdated     of FileId * string list
    | TagsSaved       of FileId * string list
    | AppError        of AppError
    | NewCatalogRequested
    | OpenCatalogRequested
    | CatalogOpened   of catalogPath: string
    | NoOp

let private defaultQuery = {
    Text = None; Tags = []; Extensions = []
    DateRange = None; SortBy = Date; SortAsc = false
}

let init () : State * Cmd<Msg> =
    let state = {
        Sidebar      = Sidebar.init ()
        Grid         = GridView.init ()
        Detail       = DetailPanel.init ()
        SearchBar    = SearchBar.init ()
        ScanProgress = None
        ActiveQuery  = defaultQuery
        Errors       = []
        IsFirstRun   = true
        CatalogPath  = None
    }
    let initCmd =
        match IsomFolio.AppPaths.readLastCatalog() with
        | None -> Cmd.none
        | Some path ->
            Cmd.OfAsync.either
                (fun () -> async {
                    IsomFolio.AppPaths.setCatalogRoot path
                    do! Db.openDatabase (IsomFolio.AppPaths.dbPath())
                    return path
                })
                ()
                CatalogOpened
                (fun ex -> AppError (DbError ex.Message))
    state, initCmd

let private runSearch (query: SearchQuery) : Cmd<Msg> =
    Cmd.OfAsync.either
        (fun () -> QueryEngine.executeSearch query)
        ()
        SearchCompleted
        (fun ex -> AppError (DbError ex.Message))

let update (msg: Msg) (state: State) : State * Cmd<Msg> =
    match msg with
    | NewCatalogRequested ->
        let cmd =
            Cmd.OfAsync.either
                (fun () -> async {
                    match window with
                    | None -> return NoOp
                    | Some w ->
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
                            IsomFolio.AppPaths.setCatalogRoot catalogPath
                            do! Db.openDatabase (IsomFolio.AppPaths.dbPath())
                            IsomFolio.AppPaths.saveLastCatalog catalogPath
                            return CatalogOpened catalogPath })
                ()
                id
                (fun ex -> AppError (ScanError ex.Message))
        state, cmd

    | OpenCatalogRequested ->
        let cmd =
            Cmd.OfAsync.either
                (fun () -> async {
                    match window with
                    | None -> return NoOp
                    | Some w ->
                        let opts = Avalonia.Platform.Storage.FolderPickerOpenOptions(Title = "Open Catalog", AllowMultiple = false)
                        let! folders = w.StorageProvider.OpenFolderPickerAsync(opts) |> Async.AwaitTask
                        if folders.Count = 0 then return NoOp
                        else
                            let catalogPath = folders[0].Path.LocalPath
                            IsomFolio.AppPaths.setCatalogRoot catalogPath
                            do! Db.openDatabase (IsomFolio.AppPaths.dbPath())
                            IsomFolio.AppPaths.saveLastCatalog catalogPath
                            return CatalogOpened catalogPath })
                ()
                id
                (fun ex -> AppError (DbError ex.Message))
        state, cmd

    | CatalogOpened path ->
        { state with CatalogPath = Some path; IsFirstRun = false }, Cmd.none

    | SidebarMsg (Sidebar.AddFolderRequested) ->
        let cmd =
            Cmd.OfAsync.either
                (fun () -> async {
                    match window with
                    | None -> return NoOp
                    | Some w ->
                        let opts = Avalonia.Platform.Storage.FolderPickerOpenOptions(Title = "Add Folder", AllowMultiple = false)
                        let! folders = w.StorageProvider.OpenFolderPickerAsync(opts) |> Async.AwaitTask
                        if folders.Count > 0 then
                            return FolderOpened (folders[0].Path.LocalPath)
                        else
                            return NoOp })
                ()
                id
                (fun ex -> AppError (ScanError ex.Message))
        state, cmd

    | SidebarMsg sbMsg ->
        let newSb = Sidebar.update sbMsg state.Sidebar
        { state with Sidebar = newSb }, Cmd.none

    | GridMsg (GridView.TileSelected fileId) ->
        let newGrid = GridView.update (GridView.TileSelected fileId) state.Grid
        let fileOpt = state.Grid.Tiles |> List.tryFind (fun t -> t.File.Id = fileId) |> Option.map (fun t -> t.File)
        let newDetail =
            match fileOpt with
            | Some f -> DetailPanel.update (DetailPanel.FileSelected f) state.Detail
            | None   -> state.Detail
        let loadTagsCmd =
            match fileOpt with
            | None   -> Cmd.none
            | Some f ->
                Cmd.OfAsync.either
                    (fun () -> Db.getTagsForFile f.Id)
                    ()
                    (fun tags -> DetailMsg (DetailPanel.TagsLoaded tags))
                    (fun ex  -> AppError (DbError ex.Message))
        { state with Grid = newGrid; Detail = newDetail }, loadTagsCmd

    | GridMsg gMsg ->
        let newGrid = GridView.update gMsg state.Grid
        { state with Grid = newGrid }, Cmd.none

    | DetailMsg (DetailPanel.AddTagRequested) ->
        let tag = state.Detail.TagInput.Trim()
        if tag = "" then state, Cmd.none
        else
            match state.Detail.File with
            | None -> state, Cmd.none
            | Some f ->
                let newDetail = DetailPanel.update (DetailPanel.TagInputChanged "") state.Detail
                let cmd =
                    Cmd.OfAsync.either
                        (fun () -> async {
                            let! result = IsomFolio.Tagging.Tagging.addTag f.Path tag
                            match result with
                            | Ok newTags ->
                                do! Db.upsertTags f.Id newTags
                                do! IsomFolio.Search.FTS.updateFileIndexTags f.Id newTags
                                return TagsSaved(f.Id, newTags)
                            | Error e -> return AppError (XmpWriteError(f.Path, e))
                        })
                        ()
                        id
                        (fun ex -> AppError (XmpWriteError(f.Path, ex.Message)))
                { state with Detail = newDetail }, cmd

    | DetailMsg (DetailPanel.RemoveTagRequested tag) ->
        match state.Detail.File with
        | None -> state, Cmd.none
        | Some f ->
            let cmd =
                Cmd.OfAsync.either
                    (fun () -> async {
                        let! result = IsomFolio.Tagging.Tagging.removeTag f.Path tag
                        match result with
                        | Ok newTags ->
                            do! Db.upsertTags f.Id newTags
                            do! IsomFolio.Search.FTS.updateFileIndexTags f.Id newTags
                            return TagsSaved(f.Id, newTags)
                        | Error e -> return AppError (XmpWriteError(f.Path, e))
                    })
                    ()
                    id
                    (fun ex -> AppError (XmpWriteError(f.Path, ex.Message)))
            state, cmd

    | DetailMsg (DetailPanel.OpenExternally) ->
        match state.Detail.File with
        | None -> state, Cmd.none
        | Some f ->
            try System.Diagnostics.Process.Start(
                    System.Diagnostics.ProcessStartInfo(f.Path, UseShellExecute = true)) |> ignore
            with _ -> ()
            state, Cmd.none

    | DetailMsg (DetailPanel.RevealInExplorer) ->
        match state.Detail.File with
        | None -> state, Cmd.none
        | Some f ->
            try
                if System.Runtime.InteropServices.RuntimeInformation.IsOSPlatform(System.Runtime.InteropServices.OSPlatform.OSX) then
                    System.Diagnostics.Process.Start("open", $"-R \"{f.Path}\"") |> ignore
                elif System.Runtime.InteropServices.RuntimeInformation.IsOSPlatform(System.Runtime.InteropServices.OSPlatform.Windows) then
                    System.Diagnostics.Process.Start("explorer", $"/select,\"{f.Path}\"") |> ignore
            with _ -> ()
            state, Cmd.none

    | DetailMsg dMsg ->
        let newDetail = DetailPanel.update dMsg state.Detail
        { state with Detail = newDetail }, Cmd.none

    | SearchBarMsg (SearchBar.QuerySubmitted txt) ->
        let query = { state.ActiveQuery with Text = if txt.Trim() = "" then None else Some txt }
        { state with ActiveQuery = query }, runSearch query

    | SearchBarMsg sbMsg ->
        let newSb = SearchBar.update sbMsg state.SearchBar
        { state with SearchBar = newSb }, Cmd.none

    | FolderOpened path ->
        let newSidebar = Sidebar.update (Sidebar.FoldersLoaded (state.Sidebar.Folders @ [ path ])) state.Sidebar
        let newState = { state with Sidebar = newSidebar; ScanProgress = Some { TotalFound = 0; Inserted = 0; FolderName = System.IO.Path.GetFileName(path) } }
        newState, Cmd.none   // Scanner wired in Phase 7

    | ScanBatchCompleted files ->
        let current = state.ScanProgress |> Option.defaultValue { TotalFound = 0; Inserted = 0; FolderName = "" }
        let progress = { current with TotalFound = current.TotalFound + files.Length; Inserted = current.Inserted + files.Length }
        let newGrid = GridView.update (GridView.TilesLoaded (state.Grid.Tiles |> List.map (fun t -> t.File) |> (@) files)) state.Grid
        { state with ScanProgress = Some progress; Grid = newGrid }, Cmd.none

    | ScanFinished _ ->
        { state with ScanProgress = None }, Cmd.none

    | SearchCompleted files ->
        let newGrid = GridView.update (GridView.TilesLoaded files) state.Grid
        { state with Grid = newGrid }, Cmd.none

    | ThumbnailReady(fileId, path) ->
        let newGrid = GridView.update (GridView.ThumbnailUpdated(fileId, Ready path)) state.Grid
        { state with Grid = newGrid }, Cmd.none

    | ThumbnailFailed(fileId, _) ->
        let newGrid = GridView.update (GridView.ThumbnailUpdated(fileId, Failed 2)) state.Grid
        { state with Grid = newGrid }, Cmd.none

    | TagsUpdated(fileId, tags) | TagsSaved(fileId, tags) ->
        let newDetail =
            if state.Detail.File |> Option.map (fun f -> f.Id) = Some fileId
            then DetailPanel.update (DetailPanel.TagsLoaded tags) state.Detail
            else state.Detail
        { state with Detail = newDetail }, Cmd.none

    | AppError err ->
        { state with Errors = err :: state.Errors |> List.truncate 5 }, Cmd.none

    | FileEventReceived _ | NoOp -> state, Cmd.none

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
            // Top bar: search + tile size controls
            DockPanel.create [
                DockPanel.dock Dock.Top
                DockPanel.background (SolidColorBrush(Color.Parse("#1E1E1E")))
                DockPanel.height 40.0
                DockPanel.children [
                    SearchBar.view state.SearchBar (SearchBarMsg >> dispatch)
                ]
            ]
            // Error banner
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
            // Left sidebar
            Sidebar.view state.Sidebar (SidebarMsg >> dispatch)
            // Right detail panel
            DetailPanel.view state.Detail (DetailMsg >> dispatch)
            // Main content area
            match state.IsFirstRun with
            | true  -> welcomeView dispatch
            | false ->
                match state.ScanProgress with
                | Some p -> progressView p
                | None   -> GridView.view state.Grid (GridMsg >> dispatch)
        ]
    ] :> Avalonia.FuncUI.Types.IView
