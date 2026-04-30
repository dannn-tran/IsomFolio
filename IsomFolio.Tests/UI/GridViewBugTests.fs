namespace IsomFolio.Tests.UI

open Xunit
open IsomFolio.Models
open IsomFolio.UI.GridView

type GridViewTests() =

    [<Fact>]
    member this.``Tile selection should remain stable when tiles are reloaded`` () =
        let fileA = { Id = "a"; Name = "A"; Path = "A"; Folder = "F"; IsOrphaned = false }
        let fileB = { Id = "b"; Name = "B"; Path = "B"; Folder = "F"; IsOrphaned = false }
        let state = { Tiles = [ { File = fileA; Thumbnail = NotRequested }; { File = fileB; Thumbnail = NotRequested } ]; TileSize = Medium; SelectedId = Some "a" }
        
        let msg = TilesLoaded [ fileB; fileA ]
        let newState = update msg state
        
        Assert.Equal(Some "a", newState.SelectedId)
[<Fact>]
member this.``Tile selection is lost if selected tile is removed from result set`` () =
    let fileA = { Id = "a"; Name = "A"; Path = "A"; Folder = "F"; IsOrphaned = false }
    let fileB = { Id = "b"; Name = "B"; Path = "B"; Folder = "F"; IsOrphaned = false }
    let state = { Tiles = [ { File = fileA; Thumbnail = NotRequested } ]; TileSize = Medium; SelectedId = Some "a" }

    // Simulate a search result that DOES NOT include the currently selected tile (e.g. it was deleted)
    let msg = TilesLoaded [ fileB ]
    let newState = update msg state

    // This should probably result in no selection, which is correct behavior.
    Assert.Equal(None, newState.SelectedId)

