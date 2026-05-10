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
                Tiles       = [ { File = fileA; Thumbnail = NotRequested }; { File = fileB; Thumbnail = NotRequested } ]
                SelectedIds = Set.singleton fileA.Id
                AnchorId    = Some fileA.Id }
        let nextState = GridView.update (GridView.TilesLoaded [ fileB; fileA ]) state
        Assert.Equal(Some fileA.Id, nextState.AnchorId)
        Assert.True(nextState.SelectedIds.Contains fileA.Id)

    [<Fact>]
    let ``selection cleared when selected tile absent from new result`` () =
        let fileA = makeFile "alpha"
        let fileB = makeFile "beta"
        let state =
            { GridView.init () with
                Tiles       = [ { File = fileA; Thumbnail = NotRequested } ]
                SelectedIds = Set.singleton fileA.Id
                AnchorId    = Some fileA.Id }
        let nextState = GridView.update (GridView.TilesLoaded [ fileB ]) state
        Assert.Equal(None, nextState.AnchorId)
        Assert.True(nextState.SelectedIds.IsEmpty)

module TileLoading =

    [<Fact>]
    let ``tiles loaded preserves existing thumbnail state for same file`` () =
        let file = makeFile "alpha"
        let startingState =
            { GridView.init () with
                Tiles       = [ { File = file; Thumbnail = Ready "/tmp/thumb.jpg" } ]
                SelectedIds = Set.singleton file.Id
                AnchorId    = Some file.Id }

        let nextState = GridView.update (GridView.TilesLoaded [ file ]) startingState

        Assert.Single(nextState.Tiles) |> ignore
        match nextState.Tiles[0].Thumbnail with
        | Ready path -> Assert.Equal("/tmp/thumb.jpg", path)
        | other -> Assert.Fail $"expected cached thumbnail to survive reload, got %A{other}"
        Assert.Equal(startingState.AnchorId, nextState.AnchorId)

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

    let private withAnchor (file: AssetFile) (state: GridView.State) =
        { state with SelectedIds = Set.singleton file.Id; AnchorId = Some file.Id }

    [<Fact>]
    let ``left moves to previous tile within a row`` () =
        let files, state = setupGrid 6
        let next = GridView.update (GridView.NavigateTo (GridView.Left, rowSize)) (withAnchor files[4] state)
        Assert.Equal(files[3].Id, next.AnchorId.Value)

    [<Fact>]
    let ``left at start of row moves to last tile of previous row`` () =
        let files, state = setupGrid 6
        let next = GridView.update (GridView.NavigateTo (GridView.Left, rowSize)) (withAnchor files[3] state)
        Assert.Equal(files[2].Id, next.AnchorId.Value)

    [<Fact>]
    let ``left at first tile stays at first tile`` () =
        let files, state = setupGrid 6
        let next = GridView.update (GridView.NavigateTo (GridView.Left, rowSize)) (withAnchor files[0] state)
        Assert.Equal(files[0].Id, next.AnchorId.Value)

    [<Fact>]
    let ``right moves to next tile within a row`` () =
        let files, state = setupGrid 6
        let next = GridView.update (GridView.NavigateTo (GridView.Right, rowSize)) (withAnchor files[1] state)
        Assert.Equal(files[2].Id, next.AnchorId.Value)

    [<Fact>]
    let ``right at end of row moves to first tile of next row`` () =
        let files, state = setupGrid 6
        let next = GridView.update (GridView.NavigateTo (GridView.Right, rowSize)) (withAnchor files[2] state)
        Assert.Equal(files[3].Id, next.AnchorId.Value)

    [<Fact>]
    let ``right at last tile stays at last tile`` () =
        let files, state = setupGrid 6
        let next = GridView.update (GridView.NavigateTo (GridView.Right, rowSize)) (withAnchor files[5] state)
        Assert.Equal(files[5].Id, next.AnchorId.Value)

    [<Fact>]
    let ``up moves to tile directly above`` () =
        let files, state = setupGrid 6
        let next = GridView.update (GridView.NavigateTo (GridView.Up, rowSize)) (withAnchor files[4] state)
        Assert.Equal(files[1].Id, next.AnchorId.Value)

    [<Fact>]
    let ``up from first row stays in first row`` () =
        let files, state = setupGrid 6
        let next = GridView.update (GridView.NavigateTo (GridView.Up, rowSize)) (withAnchor files[1] state)
        Assert.Equal(files[1].Id, next.AnchorId.Value)

    [<Fact>]
    let ``down moves to tile directly below`` () =
        let files, state = setupGrid 6
        let next = GridView.update (GridView.NavigateTo (GridView.Down, rowSize)) (withAnchor files[1] state)
        Assert.Equal(files[4].Id, next.AnchorId.Value)

    [<Fact>]
    let ``down from last row stays in last row`` () =
        let files, state = setupGrid 6
        let next = GridView.update (GridView.NavigateTo (GridView.Down, rowSize)) (withAnchor files[4] state)
        Assert.Equal(files[4].Id, next.AnchorId.Value)

    [<Fact>]
    let ``down does not jump columns when last row is partial`` () =
        // 8 tiles: row0 = 0,1,2 / row1 = 3,4,5 / row2 = 6,7 (partial)
        // tile_5 is at column 2, row 1. There is no tile directly below it in row2.
        let files, state = setupGrid 8
        let next = GridView.update (GridView.NavigateTo (GridView.Down, rowSize)) (withAnchor files[5] state)
        Assert.Equal(files[5].Id, next.AnchorId.Value)

    [<Fact>]
    let ``navigation clears multi-selection to single tile`` () =
        let files, state = setupGrid 6
        let multiState = { state with SelectedIds = Set.ofList [ files[0].Id; files[1].Id; files[2].Id ]; AnchorId = Some files[0].Id }
        let next = GridView.update (GridView.NavigateTo (GridView.Right, rowSize)) multiState
        Assert.Equal<Set<FileId>>(Set.singleton files[1].Id, next.SelectedIds)

module MultiSelect =

    let private makeGrid files =
        { GridView.init () with
            Tiles = files |> List.map (fun f -> { File = f; Thumbnail = NotRequested }) }

    [<Fact>]
    let ``plain click selects single tile and sets anchor`` () =
        let files = [ for i in 0..4 -> makeFile $"f{i}" ]
        let state = makeGrid files
        let next = GridView.update (GridView.TileClicked (files[2].Id, GridView.Plain)) state
        Assert.Equal<Set<FileId>>(Set.singleton files[2].Id, next.SelectedIds)
        Assert.Equal(Some files[2].Id, next.AnchorId)

    [<Fact>]
    let ``plain click on different tile replaces selection`` () =
        let files = [ for i in 0..4 -> makeFile $"f{i}" ]
        let state = { makeGrid files with SelectedIds = Set.singleton files[0].Id; AnchorId = Some files[0].Id }
        let next = GridView.update (GridView.TileClicked (files[3].Id, GridView.Plain)) state
        Assert.Equal<Set<FileId>>(Set.singleton files[3].Id, next.SelectedIds)
        Assert.Equal(Some files[3].Id, next.AnchorId)

    [<Fact>]
    let ``toggle adds unselected tile without clearing others`` () =
        let files = [ for i in 0..4 -> makeFile $"f{i}" ]
        let state = { makeGrid files with SelectedIds = Set.singleton files[0].Id; AnchorId = Some files[0].Id }
        let next = GridView.update (GridView.TileClicked (files[2].Id, GridView.Toggle)) state
        Assert.True(next.SelectedIds.Contains files[0].Id)
        Assert.True(next.SelectedIds.Contains files[2].Id)
        Assert.Equal(Some files[2].Id, next.AnchorId)

    [<Fact>]
    let ``toggle removes already-selected tile`` () =
        let files = [ for i in 0..4 -> makeFile $"f{i}" ]
        let state = { makeGrid files with SelectedIds = Set.ofList [ files[0].Id; files[2].Id ]; AnchorId = Some files[2].Id }
        let next = GridView.update (GridView.TileClicked (files[2].Id, GridView.Toggle)) state
        Assert.True(next.SelectedIds.Contains files[0].Id)
        Assert.False(next.SelectedIds.Contains files[2].Id)

    [<Fact>]
    let ``range extend selects contiguous tiles from anchor to clicked`` () =
        let files = [ for i in 0..4 -> makeFile $"f{i}" ]
        let state = { makeGrid files with SelectedIds = Set.singleton files[1].Id; AnchorId = Some files[1].Id }
        let next = GridView.update (GridView.TileClicked (files[3].Id, GridView.RangeExtend)) state
        Assert.Equal<Set<FileId>>(Set.ofList [ files[1].Id; files[2].Id; files[3].Id ], next.SelectedIds)

    [<Fact>]
    let ``range extend backwards from anchor`` () =
        let files = [ for i in 0..4 -> makeFile $"f{i}" ]
        let state = { makeGrid files with SelectedIds = Set.singleton files[3].Id; AnchorId = Some files[3].Id }
        let next = GridView.update (GridView.TileClicked (files[1].Id, GridView.RangeExtend)) state
        Assert.Equal<Set<FileId>>(Set.ofList [ files[1].Id; files[2].Id; files[3].Id ], next.SelectedIds)

    [<Fact>]
    let ``range extend does not move anchor`` () =
        let files = [ for i in 0..4 -> makeFile $"f{i}" ]
        let state = { makeGrid files with SelectedIds = Set.singleton files[1].Id; AnchorId = Some files[1].Id }
        let next = GridView.update (GridView.TileClicked (files[4].Id, GridView.RangeExtend)) state
        Assert.Equal(Some files[1].Id, next.AnchorId)

    [<Fact>]
    let ``range extend with no anchor defaults to first tile`` () =
        let files = [ for i in 0..4 -> makeFile $"f{i}" ]
        let state = makeGrid files
        let next = GridView.update (GridView.TileClicked (files[2].Id, GridView.RangeExtend)) state
        Assert.Equal<Set<FileId>>(Set.ofList [ files[0].Id; files[1].Id; files[2].Id ], next.SelectedIds)

module Albums =

    let private makeAlbum id name : Album =
        { Id = id; Name = name; Kind = Manual; SortOrder = 0 }

    [<Fact>]
    let ``AlbumsUpdated sets Albums`` () =
        let album = makeAlbum "a1" "Tokyo"
        let next = GridView.update (GridView.AlbumsUpdated [album]) (GridView.init ())
        Assert.Equal<Album list>([album], next.Albums)

    [<Fact>]
    let ``CurrentAlbumChanged Some sets CurrentAlbumId`` () =
        let next = GridView.update (GridView.CurrentAlbumChanged (Some "a1")) (GridView.init ())
        Assert.Equal(Some "a1", next.CurrentAlbumId)

    [<Fact>]
    let ``CurrentAlbumChanged None clears CurrentAlbumId`` () =
        let state = { GridView.init () with CurrentAlbumId = Some "a1" }
        let next = GridView.update (GridView.CurrentAlbumChanged None) state
        Assert.Equal(None, next.CurrentAlbumId)

    [<Fact>]
    let ``AddToAlbum does not mutate state`` () =
        let file = makeFile "alpha"
        let state =
            { GridView.init () with
                Tiles       = [{ File = file; Thumbnail = NotRequested }]
                SelectedIds = Set.singleton file.Id
                AnchorId    = Some file.Id }
        let next = GridView.update (GridView.AddToAlbum (file.Id, "a1")) state
        Assert.Equal<GridView.TileModel list>(state.Tiles, next.Tiles)
        Assert.Equal(state.AnchorId, next.AnchorId)

    [<Fact>]
    let ``RemoveFromAlbum does not mutate state`` () =
        let file = makeFile "alpha"
        let state =
            { GridView.init () with
                Tiles       = [{ File = file; Thumbnail = NotRequested }]
                SelectedIds = Set.singleton file.Id
                AnchorId    = Some file.Id }
        let next = GridView.update (GridView.RemoveFromAlbum (file.Id, "a1")) state
        Assert.Equal<GridView.TileModel list>(state.Tiles, next.Tiles)
        Assert.Equal(state.AnchorId, next.AnchorId)
