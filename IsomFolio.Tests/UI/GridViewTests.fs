module IsomFolio.Tests.UI.GridViewTests

open System
open Xunit
open IsomFolio.Models
open IsomFolio.UI

let private makeFile (name: string) : AssetFile =
    let path = $"/photos/{name}.jpg"
    {
        Id = IsomFolio.FileIndex.computeFileId path
        Path = path
        Name = $"{name}.jpg"
        Folder = "/photos"
        Ext = "jpg"
        SizeBytes = 1024L
        MTimeUnix = DateTimeOffset.UtcNow.ToUnixTimeSeconds()
        IsOrphaned = false
        OrphanedAt = None
    }

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
        Assert.Equal<FileId option>(None, nextState.SelectedId)

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
