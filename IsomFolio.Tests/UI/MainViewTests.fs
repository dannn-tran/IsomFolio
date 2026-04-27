module IsomFolio.Tests.UI.MainViewTests

open System
open System.IO
open Xunit
open Avalonia.Controls
open IsomFolio.Models
open IsomFolio.Storage
open IsomFolio.UI

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

let private makeState conn : MainView.State = {
    Sidebar = Sidebar.init ()
    Grid = GridView.init ()
    Detail = DetailPanel.init ()
    SearchBar = SearchBar.init ()
    ScanProgress = None
    ActiveQuery = defaultQuery
    Notifications = []
    OrphanCount = 0
    IsFirstRun = false
    Catalog = MainView.OpenedCatalog("/catalog", conn)
    Window = Unchecked.defaultof<Window>
}

let private makeFile (name: string) (folder: string) : AssetFile =
    let path = $"{folder}/{name}.jpg"
    {
        Id = IsomFolio.FileIndex.computeFileId path
        Path = path
        Name = $"{name}.jpg"
        Folder = folder
        Ext = "jpg"
        SizeBytes = 1024L
        MTimeUnix = DateTimeOffset.UtcNow.ToUnixTimeSeconds()
        IsOrphaned = false
        OrphanedAt = None
    }

module FolderSelection =

    [<Fact>]
    let ``folder click updates active query`` () =
        async {
            let! conn = openTestDb ()
            let state = makeState conn
            let nextState, _ =
                MainView.update
                    (MainView.SidebarMsg (Sidebar.FolderSelected "/outer/inner"))
                    state

            Assert.Equal<string option>(Some "/outer/inner", nextState.ActiveQuery.FolderPath)
            Assert.Equal<string option>(Some "/outer/inner", nextState.Sidebar.SelectedFolder)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``clicking selected folder clears folder filter`` () =
        async {
            let! conn = openTestDb ()
            let state =
                { makeState conn with
                    Sidebar = { Sidebar.init () with SelectedFolder = Some "/outer/inner" }
                    ActiveQuery = { defaultQuery with FolderPath = Some "/outer/inner" } }
            let nextState, _ =
                MainView.update
                    (MainView.SidebarMsg (Sidebar.FolderSelected "/outer/inner"))
                    state

            Assert.Equal<string option>(None, nextState.ActiveQuery.FolderPath)
            Assert.Equal<string option>(None, nextState.Sidebar.SelectedFolder)
        } |> Async.RunSynchronously

module ScanState =

    [<Fact>]
    let ``scan progress updated stores latest progress state`` () =
        async {
            let! conn = openTestDb ()
            let state = makeState conn
            let progress = { TotalFound = 125; Inserted = 125; FolderName = "Imports" }
            let nextState, _ = MainView.update (MainView.ScanProgressUpdated progress) state

            Assert.Equal<ScanProgress option>(Some progress, nextState.ScanProgress)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``scan batch does not bypass active folder filter`` () =
        async {
            let! conn = openTestDb ()
            let visible = makeFile "visible" "/outer"
            let unrelated = makeFile "other" "/other"
            let state =
                { makeState conn with
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
            let existing = makeFile "existing" "/outer"
            let incoming = makeFile "incoming" "/outer"
            let state =
                { makeState conn with
                    Grid = { GridView.init () with Tiles = [ { File = existing; Thumbnail = Ready "/tmp/existing.jpg" } ] } }

            let nextState, _ = MainView.update (MainView.ScanBatchCompleted [ incoming ]) state

            Assert.Equal(2, nextState.Grid.Tiles.Length)
            Assert.Contains(nextState.Grid.Tiles, fun tile -> tile.File.Id = existing.Id)
            Assert.Contains(nextState.Grid.Tiles, fun tile -> tile.File.Id = incoming.Id)
        } |> Async.RunSynchronously
