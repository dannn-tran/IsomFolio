module IsomFolio.Tests.UI.SmartAlbumEditorTests

open System
open Xunit
open IsomFolio.Core.Models
open IsomFolio.UI.SmartAlbumEditor

let private defaultQuery = {
    Text = None; FolderPath = None; Tags = []; Extensions = []
    DateRange = None; SortBy = Date; SortAsc = false
}

let private makeSmartAlbum id name (q: SearchQuery) : Album =
    { Id = id; Name = name; Kind = Smart q; SortOrder = 0 }

module InitFromAlbum =

    [<Fact>]
    let ``sets AlbumId and AlbumName`` () =
        let album = makeSmartAlbum "a1" "By Date" defaultQuery
        let state = initFromAlbum album
        Assert.Equal("a1", state.AlbumId)
        Assert.Equal("By Date", state.AlbumName)

    [<Fact>]
    let ``loads tags from query`` () =
        let q = { defaultQuery with Tags = [ "travel"; "paris" ] }
        let state = initFromAlbum (makeSmartAlbum "a1" "N" q)
        Assert.Equal<string list>([ "travel"; "paris" ], state.TagFilter)

    [<Fact>]
    let ``loads extensions from query`` () =
        let q = { defaultQuery with Extensions = [ "jpg"; "raw" ] }
        let state = initFromAlbum (makeSmartAlbum "a1" "N" q)
        Assert.Equal<string list>([ "jpg"; "raw" ], state.ExtFilter)

    [<Fact>]
    let ``loads date range from query`` () =
        let q = { defaultQuery with DateRange = Some (DateTime(2024, 1, 1), DateTime(2024, 12, 31)) }
        let state = initFromAlbum (makeSmartAlbum "a1" "N" q)
        Assert.Equal("2024-01-01", state.DateFrom)
        Assert.Equal("2024-12-31", state.DateTo)

    [<Fact>]
    let ``no date range gives empty strings`` () =
        let state = initFromAlbum (makeSmartAlbum "a1" "N" defaultQuery)
        Assert.Equal("", state.DateFrom)
        Assert.Equal("", state.DateTo)

    [<Fact>]
    let ``loads folder from query`` () =
        let q = { defaultQuery with FolderPath = Some "/photos" }
        let state = initFromAlbum (makeSmartAlbum "a1" "N" q)
        Assert.Equal(Some "/photos", state.FolderFilter)

module ToSearchQuery =

    [<Fact>]
    let ``empty criteria produces minimal query`` () =
        let state = initFromAlbum (makeSmartAlbum "a1" "N" defaultQuery)
        let q = toSearchQuery state
        Assert.Equal(None, q.Text)
        Assert.Equal(None, q.FolderPath)
        Assert.Empty(q.Tags)
        Assert.Empty(q.Extensions)
        Assert.Equal(None, q.DateRange)

    [<Fact>]
    let ``tags round-trip through toSearchQuery`` () =
        let original = { defaultQuery with Tags = [ "travel"; "paris" ] }
        let state = initFromAlbum (makeSmartAlbum "a1" "N" original)
        let result = toSearchQuery state
        Assert.Equal<string list>([ "travel"; "paris" ], result.Tags)

    [<Fact>]
    let ``extensions round-trip through toSearchQuery`` () =
        let original = { defaultQuery with Extensions = [ "jpg"; "raw" ] }
        let state = initFromAlbum (makeSmartAlbum "a1" "N" original)
        let result = toSearchQuery state
        Assert.Equal<string list>([ "jpg"; "raw" ], result.Extensions)

    [<Fact>]
    let ``date range round-trip through toSearchQuery`` () =
        let d1 = DateTime(2024, 1, 1)
        let d2 = DateTime(2024, 12, 31)
        let original = { defaultQuery with DateRange = Some (d1, d2) }
        let state = initFromAlbum (makeSmartAlbum "a1" "N" original)
        let result = toSearchQuery state
        Assert.Equal(Some (d1, d2), result.DateRange)

    [<Fact>]
    let ``partial date from only expands to MinValue–date`` () =
        let state = initFromAlbum (makeSmartAlbum "a1" "N" defaultQuery)
        let state = { state with DateFrom = ""; DateTo = "2024-06-30" }
        let result = toSearchQuery state
        match result.DateRange with
        | Some (f, t) ->
            Assert.Equal(DateTime.MinValue, f)
            Assert.Equal(DateTime(2024, 6, 30), t)
        | None -> Assert.Fail "expected DateRange to be Some"

module UpdateMsg =

    [<Fact>]
    let ``TagAdded appends tag`` () =
        let state = initFromAlbum (makeSmartAlbum "a1" "N" defaultQuery)
        let next = update (TagAdded "travel") state
        Assert.Contains("travel", next.TagFilter)

    [<Fact>]
    let ``TagRemoved removes tag`` () =
        let q = { defaultQuery with Tags = [ "travel"; "paris" ] }
        let state = initFromAlbum (makeSmartAlbum "a1" "N" q)
        let next = update (TagRemoved "travel") state
        Assert.Equal<string list>([ "paris" ], next.TagFilter)

    [<Fact>]
    let ``SaveRequested does not mutate state`` () =
        let state = initFromAlbum (makeSmartAlbum "a1" "N" defaultQuery)
        let next = update SaveRequested state
        Assert.Equal(state, next)

    [<Fact>]
    let ``Cancelled does not mutate state`` () =
        let state = initFromAlbum (makeSmartAlbum "a1" "N" defaultQuery)
        let next = update Cancelled state
        Assert.Equal(state, next)
