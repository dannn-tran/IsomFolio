module IsomFolio.Tests.UI.GridViewTests

open System
open Xunit
open IsomFolio.Core.Models
open IsomFolio.UI

let private makeFile (name: string) : AssetFile =
    let path = $"/photos/{name}.jpg"
    {
        Id            = IsomFolio.Core.FileIndex.computeFileId path
        Path          = path
        Name          = $"{name}.jpg"
        Folder        = "/photos"
        Ext           = "jpg"
        SizeBytes     = 1024L
        MTimeUnix     = DateTimeOffset.UtcNow.ToUnixTimeSeconds()
        CreatedAtUnix = DateTimeOffset.UtcNow.ToUnixTimeSeconds()
        IsOrphaned    = false
        OrphanedAt    = None
    }

module SelectionStability =

    [<Fact>]
    let ``selection preserved when tiles reloaded in different order`` () =
        let fileA = makeFile "alpha"
        let fileB = makeFile "beta"
        let state =
            { GridView.init () with
                Tiles = [ { File = fileA; Thumbnail = NotRequested }; { File = fileB; Thumbnail = NotRequested } ]
                SelectedId = Some fileA.Id }
        let nextState = GridView.update (GridView.TilesLoaded [ fileB; fileA ]) state
        Assert.Equal(Some fileA.Id, nextState.SelectedId)

    [<Fact>]
    let ``selection cleared when selected tile absent from new result`` () =
        let fileA = makeFile "alpha"
        let fileB = makeFile "beta"
        let state =
            { GridView.init () with
                Tiles = [ { File = fileA; Thumbnail = NotRequested } ]
                SelectedId = Some fileA.Id }
        let nextState = GridView.update (GridView.TilesLoaded [ fileB ]) state
        Assert.Equal(None, nextState.SelectedId)

module TileLoading =

    [<Fact>]
    let ``tiles loaded preserves existing thumbnail state for same file`` () =
        let file = makeFile "alpha"
        let startingState =
            { GridView.init () with
                Tiles = [ { File = file; Thumbnail = Ready "/tmp/thumb.jpg" } ]
                SelectedId = Some file.Id }

        let nextState = GridView.update (GridView.TilesLoaded [ file ]) startingState

        Assert.Single(nextState.Tiles) |> ignore
        match nextState.Tiles[0].Thumbnail with
        | Ready path -> Assert.Equal("/tmp/thumb.jpg", path)
        | other -> Assert.Fail $"expected cached thumbnail to survive reload, got %A{other}"
        Assert.Equal(startingState.SelectedId, nextState.SelectedId)

    [<Fact>]
    let ``tiles loaded initializes new file as not requested`` () =
        let existing = makeFile "alpha"
        let incoming = makeFile "beta"
        let startingState =
            { GridView.init () with
                Tiles = [ { File = existing; Thumbnail = Ready "/tmp/thumb.jpg" } ] }

        let nextState = GridView.update (GridView.TilesLoaded [ existing; incoming ]) startingState

        Assert.Equal(2, nextState.Tiles.Length)
        match nextState.Tiles[0].Thumbnail with
        | Ready _ -> ()
        | other -> Assert.Fail $"expected existing thumbnail state to be preserved, got %A{other}"
        match nextState.Tiles[1].Thumbnail with
        | NotRequested -> ()
        | other -> Assert.Fail $"expected new tile to start at NotRequested, got %A{other}"

// Navigation tests use an explicit rowSize rather than deriving it from GridWidth,
// since GridWidth was removed from GridView.State.
// All tests below use rowSize = 3 giving layout: tile_0 tile_1 tile_2 / tile_3 tile_4 tile_5
module KeyboardNavigation =
    let private rowSize = 3

    let private setupGrid tileCount =
        let files = [ for i in 0 .. tileCount - 1 -> makeFile $"tile_{i}" ]
        let state =
            { GridView.init () with
                Tiles = files |> List.map (fun f -> { File = f; Thumbnail = NotRequested }) }
        files, state

    [<Fact>]
    let ``left moves to previous tile within a row`` () =
        let files, state = setupGrid 6
        let state = { state with SelectedId = Some files[4].Id }
        let next = GridView.update (GridView.NavigateTo (GridView.Left, rowSize)) state
        Assert.Equal(files[3].Id, next.SelectedId.Value)

    [<Fact>]
    let ``left at start of row moves to last tile of previous row`` () =
        let files, state = setupGrid 6
        let state = { state with SelectedId = Some files[3].Id }
        let next = GridView.update (GridView.NavigateTo (GridView.Left, rowSize)) state
        Assert.Equal(files[2].Id, next.SelectedId.Value)

    [<Fact>]
    let ``left at first tile stays at first tile`` () =
        let files, state = setupGrid 6
        let state = { state with SelectedId = Some files[0].Id }
        let next = GridView.update (GridView.NavigateTo (GridView.Left, rowSize)) state
        Assert.Equal(files[0].Id, next.SelectedId.Value)

    [<Fact>]
    let ``right moves to next tile within a row`` () =
        let files, state = setupGrid 6
        let state = { state with SelectedId = Some files[1].Id }
        let next = GridView.update (GridView.NavigateTo (GridView.Right, rowSize)) state
        Assert.Equal(files[2].Id, next.SelectedId.Value)

    [<Fact>]
    let ``right at end of row moves to first tile of next row`` () =
        let files, state = setupGrid 6
        let state = { state with SelectedId = Some files[2].Id }
        let next = GridView.update (GridView.NavigateTo (GridView.Right, rowSize)) state
        Assert.Equal(files[3].Id, next.SelectedId.Value)

    [<Fact>]
    let ``right at last tile stays at last tile`` () =
        let files, state = setupGrid 6
        let state = { state with SelectedId = Some files[5].Id }
        let next = GridView.update (GridView.NavigateTo (GridView.Right, rowSize)) state
        Assert.Equal(files[5].Id, next.SelectedId.Value)

    [<Fact>]
    let ``up moves to tile directly above`` () =
        let files, state = setupGrid 6
        let state = { state with SelectedId = Some files[4].Id }
        let next = GridView.update (GridView.NavigateTo (GridView.Up, rowSize)) state
        Assert.Equal(files[1].Id, next.SelectedId.Value)

    [<Fact>]
    let ``up from first row stays in first row`` () =
        let files, state = setupGrid 6
        let state = { state with SelectedId = Some files[1].Id }
        let next = GridView.update (GridView.NavigateTo (GridView.Up, rowSize)) state
        Assert.Equal(files[1].Id, next.SelectedId.Value)

    [<Fact>]
    let ``down moves to tile directly below`` () =
        let files, state = setupGrid 6
        let state = { state with SelectedId = Some files[1].Id }
        let next = GridView.update (GridView.NavigateTo (GridView.Down, rowSize)) state
        Assert.Equal(files[4].Id, next.SelectedId.Value)

    [<Fact>]
    let ``down from last row stays in last row`` () =
        let files, state = setupGrid 6
        let state = { state with SelectedId = Some files[4].Id }
        let next = GridView.update (GridView.NavigateTo (GridView.Down, rowSize)) state
        Assert.Equal(files[4].Id, next.SelectedId.Value)

    [<Fact>]
    let ``down does not jump columns when last row is partial`` () =
        // 8 tiles: row0 = 0,1,2 / row1 = 3,4,5 / row2 = 6,7 (partial)
        // tile_5 is at column 2, row 1. There is no tile directly below it in row2.
        let files, state = setupGrid 8
        let state = { state with SelectedId = Some files[5].Id }
        let next = GridView.update (GridView.NavigateTo (GridView.Down, rowSize)) state
        Assert.Equal(files[5].Id, next.SelectedId.Value)
