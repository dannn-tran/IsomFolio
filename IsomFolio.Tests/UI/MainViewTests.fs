module IsomFolio.App.Tests.UI.MainViewTests

open System
open System.IO
open System.Collections.Concurrent
open System.Threading.Tasks
open IsomFolio.Core.Indexing.Types
open IsomFolio.UI
open Xunit
open Elmish
open IsomFolio.Core.Models
open IsomFolio.Core.Storage

let private defaultQuery = {
    Text = None
    FolderPath = None
    Tags = []
    Extensions = []
    DateRange = None
    SortBy = Date
    SortAsc = false
}

let private openTestDb () =
    async {
        let dbPath = Path.Combine(Path.GetTempPath(), $"isomfolio_mainview_{Guid.NewGuid():N}.db")
        return! Db.openDatabase dbPath
    }

let private openTestCatalog () =
    async {
        let catalogPath = Path.Combine(Path.GetTempPath(), $"isomfolio_mainview_{Guid.NewGuid():N}.isomfolio")
        Directory.CreateDirectory(catalogPath) |> ignore
        let! conn = Db.openDatabase (IsomFolio.Core.AppPaths.dbPath catalogPath)
        return catalogPath, conn
    }

let private makeState catalogPath : MainView.State = {
    Sidebar         = Sidebar.init ()
    Grid            = GridView.init ()
    Detail          = DetailPanel.init ()
    SearchBar       = SearchBar.init ()
    ScanProgress    = None
    ActiveQuery     = defaultQuery
    Notifications   = []
    OrphanCount     = 0
    IsFirstRun      = false
    Catalog         = MainView.OpenedCatalog(catalogPath)
    SearchRequestId = 0
    PendingFolders  = Set.empty
}

let private makeFile (name: string) (folder: string) : AssetFile =
    let path = $"{folder}/{name}.jpg"
    {
        Id = IsomFolio.Core.FileIndex.computeFileId path
        Path = path
        Name = $"{name}.jpg"
        Folder = folder
        Ext = "jpg"
        SizeBytes = 1024L
        MTimeUnix = DateTimeOffset.UtcNow.ToUnixTimeSeconds()
        IsOrphaned = false
        OrphanedAt = None
    }

let private execCmd (cmd: Cmd<MainView.Msg>) =
    async {
        let messages = ConcurrentQueue<MainView.Msg>()
        let tcs = TaskCompletionSource()
        let dispatch msg =
            messages.Enqueue(msg)
            tcs.TrySetResult() |> ignore

        for sub in cmd do
            sub dispatch

        let! completed = tcs.Task.WaitAsync(TimeSpan.FromSeconds(2.0)) |> Async.AwaitTask
        let _ = completed
        return messages.ToArray() |> Array.toList
    }

module FolderSelection =

    [<Fact>]
    let ``folder click updates active query`` () =
        async {
            let! conn = openTestDb ()
            use c = conn
            let state = makeState "/catalog"
            let nextState, _ =
                MainView.update
                    (MainView.SidebarMsg (Sidebar.FolderSelected "/outer/inner"))
                    state

            Assert.Equal<string option>(Some "/outer/inner", nextState.ActiveQuery.FolderPath)
            Assert.Equal<string option>(Some "/outer/inner", nextState.Sidebar.SelectedFolder)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``clicking selected folder keeps selection`` () =
        async {
            let! conn = openTestDb ()
            use c = conn
            let state =
                { makeState "/catalog" with
                    Sidebar = { Sidebar.init () with SelectedFolder = Some "/outer/inner" }
                    ActiveQuery = { defaultQuery with FolderPath = Some "/outer/inner" } }
            let nextState, _ =
                MainView.update
                    (MainView.SidebarMsg (Sidebar.FolderSelected "/outer/inner"))
                    state

            Assert.Equal<string option>(Some "/outer/inner", nextState.ActiveQuery.FolderPath)
            Assert.Equal<string option>(Some "/outer/inner", nextState.Sidebar.SelectedFolder)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``folder click dispatches filtered search results`` () =
        async {
            let! catalogPath, conn = openTestCatalog ()
            let root = makeFile "root" "/outer"
            let nested = makeFile "nested" "/outer/inner"
            let other = makeFile "other" "/other"
            let! _ = Db.upsertFiles conn [ root; nested; other ]

            conn.Dispose()
            let state = makeState catalogPath
            let _, cmd =
                MainView.update
                    (MainView.SidebarMsg (Sidebar.FolderSelected "/outer"))
                    state

            let! messages = execCmd cmd
            let searchCompleted =
                messages
                |> List.choose (function MainView.SearchCompleted(_, files) -> Some files | _ -> None)
                |> List.tryHead

            Assert.True(searchCompleted.IsSome)
            let ids = searchCompleted.Value |> List.map _.Id |> Set.ofList
            Assert.Equal(2, ids.Count)
            Assert.Contains(root.Id, ids)
            Assert.Contains(nested.Id, ids)
        } |> Async.RunSynchronously

module ScanState =

    [<Fact>]
    let ``scan progress updated stores latest progress state`` () =
        async {
            let! conn = openTestDb ()
            use c = conn
            let state = makeState "/catalog"
            let progress = { TotalFound = 125; Inserted = 125; FolderName = "Imports" }
            let nextState, _ = MainView.update (MainView.ScanProgressUpdated progress) state

            Assert.Equal<ScanProgress option>(Some progress, nextState.ScanProgress)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``scan batch does not bypass active folder filter`` () =
        async {
            let! conn = openTestDb ()
            use c = conn
            let visible = makeFile "visible" "/outer"
            let unrelated = makeFile "other" "/other"
            let state =
                { makeState "/catalog" with
                    ActiveQuery = { defaultQuery with FolderPath = Some "/outer" }
                    Grid = { GridView.init () with Tiles = [ { File = visible; Thumbnail = Ready "/tmp/visible.jpg" } ] } }

            let nextState, _ = MainView.update (MainView.ScanBatchCompleted [ unrelated ]) state

            Assert.Single(nextState.Grid.Tiles) |> ignore
            Assert.Equal(visible.Id, nextState.Grid.Tiles[0].File.Id)
            match nextState.Grid.Tiles[0].Thumbnail with
            | Ready path -> Assert.Equal("/tmp/visible.jpg", path)
            | other -> Assert.Fail $"expected filtered tile to remain intact, got %A{other}"
        } |> Async.RunSynchronously

    [<Fact>]
    let ``scan batch appends files when no filter is active`` () =
        async {
            let! conn = openTestDb ()
            use c = conn
            let existing = makeFile "existing" "/outer"
            let incoming = makeFile "incoming" "/outer"
            let state =
                { makeState "/catalog" with
                    Grid = { GridView.init () with Tiles = [ { File = existing; Thumbnail = Ready "/tmp/existing.jpg" } ] } }

            let nextState, _ = MainView.update (MainView.ScanBatchCompleted [ incoming ]) state

            Assert.Equal(2, nextState.Grid.Tiles.Length)
            Assert.Contains(nextState.Grid.Tiles, fun tile -> tile.File.Id = existing.Id)
            Assert.Contains(nextState.Grid.Tiles, fun tile -> tile.File.Id = incoming.Id)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``scan batch places new files after existing tiles so existing positions are stable`` () =
        async {
            let! conn = openTestDb ()
            use c = conn
            let existing = makeFile "existing" "/outer"
            let incoming = makeFile "incoming" "/outer"
            let state =
                { makeState "/catalog" with
                    Grid = { GridView.init () with Tiles = [ { File = existing; Thumbnail = Ready "/tmp/existing.jpg" } ] } }

            let nextState, _ = MainView.update (MainView.ScanBatchCompleted [ incoming ]) state

            Assert.Equal(2, nextState.Grid.Tiles.Length)
            Assert.Equal(existing.Id, nextState.Grid.Tiles[0].File.Id)
            Assert.Equal(incoming.Id, nextState.Grid.Tiles[1].File.Id)
        } |> Async.RunSynchronously

module ThumbnailLifecycle =

    // Repro for "stuck progress bar" bug — adding a 2nd folder leaves new tiles Pending
    [<Fact>]
    let ``thumbnail update after second batch transitions tile to Ready`` () =
        async {
            let! conn = openTestDb ()
            use c = conn

            // Folder 1 already finished — tiles are Ready
            let f1a = makeFile "f1a" "/folder1"
            let f1b = makeFile "f1b" "/folder1"
            let folder1Tiles =
                [ ({ File = f1a; Thumbnail = Ready "/cache/f1a.jpg" } : GridView.TileModel)
                  { File = f1b; Thumbnail = Ready "/cache/f1b.jpg" } ]
            let state =
                { makeState "/catalog" with
                    Grid = { GridView.init () with Tiles = folder1Tiles } }

            // 2nd folder batch arrives
            let f2a = makeFile "f2a" "/folder2"
            let f2b = makeFile "f2b" "/folder2"
            let stateAfterScan, _ = MainView.update (MainView.ScanBatchCompleted [ f2a; f2b ]) state

            // Sanity: folder 1 tiles still Ready, folder 2 tiles Pending
            Assert.Equal(4, stateAfterScan.Grid.Tiles.Length)
            let pendingIds =
                stateAfterScan.Grid.Tiles
                |> List.filter (fun t -> t.Thumbnail = Pending)
                |> List.map _.File.Id
                |> Set.ofList
            Assert.Equal<Set<string>>(Set.ofList [ f2a.Id; f2b.Id ], pendingIds)

            // Worker callback: simulate ThumbnailUpdated for the new files
            let stateAfterReady1, _ =
                MainView.update
                    (MainView.GridMsg (GridView.ThumbnailUpdated(f2a.Id, Ready "/cache/f2a.jpg")))
                    stateAfterScan
            let stateAfterReady2, _ =
                MainView.update
                    (MainView.GridMsg (GridView.ThumbnailUpdated(f2b.Id, Ready "/cache/f2b.jpg")))
                    stateAfterReady1

            let f2aTile = stateAfterReady2.Grid.Tiles |> List.find (fun t -> t.File.Id = f2a.Id)
            let f2bTile = stateAfterReady2.Grid.Tiles |> List.find (fun t -> t.File.Id = f2b.Id)
            match f2aTile.Thumbnail with
            | Ready _ -> ()
            | other -> Assert.Fail $"expected f2a to be Ready, got %A{other}"
            match f2bTile.Thumbnail with
            | Ready _ -> ()
            | other -> Assert.Fail $"expected f2b to be Ready, got %A{other}"
        } |> Async.RunSynchronously

module PendingFolders =

    [<Fact>]
    let ``Created event adds parent folder to PendingFolders`` () =
        let state = makeState "/catalog"
        let nextState, _ =
            MainView.update (MainView.FileEventReceived (Created "/photos/summer/beach.jpg")) state
        Assert.Contains("/photos/summer", nextState.PendingFolders)

    [<Fact>]
    let ``Modified event adds parent folder to PendingFolders`` () =
        let state = makeState "/catalog"
        let nextState, _ =
            MainView.update (MainView.FileEventReceived (Modified "/photos/summer/beach.jpg")) state
        Assert.Contains("/photos/summer", nextState.PendingFolders)

    [<Fact>]
    let ``Renamed event does not add to PendingFolders`` () =
        let state = makeState "/catalog"
        let nextState, _ =
            MainView.update (MainView.FileEventReceived (Renamed("/photos/old.jpg", "/photos/archive/new.jpg"))) state
        Assert.Empty(nextState.PendingFolders)

    [<Fact>]
    let ``Deleted event does not add to PendingFolders`` () =
        let state = makeState "/catalog"
        let nextState, _ =
            MainView.update (MainView.FileEventReceived (Deleted "/photos/summer/beach.jpg")) state
        Assert.Empty(nextState.PendingFolders)

    [<Fact>]
    let ``unsupported extension Created event does not add to PendingFolders`` () =
        let state = makeState "/catalog"
        let nextState, _ =
            MainView.update (MainView.FileEventReceived (Created "/photos/document.pdf")) state
        Assert.Empty(nextState.PendingFolders)

    [<Fact>]
    let ``FolderResynced removes exact folder and all descendants`` () =
        let state =
            { makeState "/catalog" with
                PendingFolders = Set.ofList [ "/photos/summer"; "/photos/winter/raw"; "/other" ] }
        let nextState, _ = MainView.update (MainView.FolderResynced "/photos") state
        Assert.DoesNotContain("/photos/summer", nextState.PendingFolders)
        Assert.DoesNotContain("/photos/winter/raw", nextState.PendingFolders)
        Assert.Contains("/other", nextState.PendingFolders)

    [<Fact>]
    let ``FolderResynced removes exact folder match`` () =
        let state =
            { makeState "/catalog" with
                PendingFolders = Set.ofList [ "/photos/summer"; "/other" ] }
        let nextState, _ = MainView.update (MainView.FolderResynced "/photos/summer") state
        Assert.DoesNotContain("/photos/summer", nextState.PendingFolders)
        Assert.Contains("/other", nextState.PendingFolders)

module SidebarFolderRemoval =

    [<Fact>]
    let ``removing a virtual parent removes all child folders`` () =
        let state = { Sidebar.init () with Folders = [ "/a/b"; "/a/c" ] }
        let nextState = Sidebar.update (Sidebar.FolderRemoved "/a") state
        Assert.Empty(nextState.Folders)

    [<Fact>]
    let ``removing an exact folder does not touch siblings`` () =
        let state = { Sidebar.init () with Folders = [ "/a/b"; "/a/c" ] }
        let nextState = Sidebar.update (Sidebar.FolderRemoved "/a/b") state
        Assert.Equal<string list>([ "/a/c" ], nextState.Folders)

module KeyboardNavigation =

    [<Fact>]
    let ``keyboard navigation updates detail panel`` () =
        let f1 = makeFile "f1" "/photos"
        let f2 = makeFile "f2" "/photos"
        let state =
            { makeState "/catalog" with
                Grid =
                    { GridView.init () with
                        Tiles     = [ { File = f1; Thumbnail = NotRequested }; { File = f2; Thumbnail = NotRequested } ]
                        SelectedId = Some f1.Id } }

        let nextState, _ = MainView.update (MainView.GridMsg (GridView.NavigateTo (GridView.Right, 2))) state

        Assert.Equal(f2.Id, nextState.Grid.SelectedId.Value)
        Assert.Equal(Some f2.Id, nextState.Detail.File |> Option.map _.Id)

    [<Fact>]
    let ``keyboard navigation at boundary does not move selection or clear detail`` () =
        let f1 = makeFile "f1" "/photos"
        let f2 = makeFile "f2" "/photos"
        let state =
            { makeState "/catalog" with
                Grid =
                    { GridView.init () with
                        Tiles     = [ { File = f1; Thumbnail = NotRequested }; { File = f2; Thumbnail = NotRequested } ]
                        SelectedId = Some f2.Id }
                Detail = DetailPanel.update (DetailPanel.FileSelected f2) (DetailPanel.init ()) }

        let nextState, _ = MainView.update (MainView.GridMsg (GridView.NavigateTo (GridView.Right, 2))) state

        Assert.Equal(f2.Id, nextState.Grid.SelectedId.Value)
        Assert.Equal(Some f2.Id, nextState.Detail.File |> Option.map _.Id)
