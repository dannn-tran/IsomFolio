module IsomFolio.Tests.Search.QueryEngineTests

open System
open System.IO
open Xunit
open IsomFolio.Models
open IsomFolio.Storage
open IsomFolio.Search

let private openTestDb () =
    async {
        let dbPath = Path.Combine(Path.GetTempPath(), $"isomfolio_search_{Guid.NewGuid():N}.db")
        return! Db.openDatabase dbPath
    }

let private makeFile (name: string) (folder: string) (ext: string) : AssetFile =
    let path = $"{folder}/{name}.{ext}"
    {
        Id         = IsomFolio.FileIndex.computeFileId path
        Path       = path
        Name       = $"{name}.{ext}"
        Folder     = folder
        Ext        = ext
        SizeBytes  = 1024L
        MTimeUnix  = DateTimeOffset.UtcNow.ToUnixTimeSeconds()
        IsOrphaned = false
        OrphanedAt = None
    }

let private defaultQuery = {
    Text       = None
    FolderPath = None
    Tags       = []
    Extensions = []
    DateRange  = None
    SortBy     = Name
    SortAsc    = true
}


module SanitizeFtsQuery =

    [<Fact>]
    let ``appends prefix wildcard`` () =
        Assert.Equal("hello*", FTS.sanitizeFtsQuery "hello")

    [<Fact>]
    let ``trailing space suppresses wildcard`` () =
        Assert.Equal("hello", FTS.sanitizeFtsQuery "hello ")

    [<Fact>]
    let ``replaces special chars with spaces to preserve word boundaries`` () =
        Assert.Equal("hello world*", FTS.sanitizeFtsQuery "hello(world)")

    [<Fact>]
    let ``blank input returns empty string`` () =
        Assert.Equal("", FTS.sanitizeFtsQuery "   ")


module ExecuteSearch =

    [<Fact>]
    let ``returns all non-orphaned files when no filters`` () =
        async {
            let! c = openTestDb ()
            let! _ = Db.upsertFiles c [ makeFile "alpha" "/photos" "jpg"; makeFile "beta" "/photos" "png" ]
            let! results = QueryEngine.executeSearch c defaultQuery
            Assert.Equal(2, results.Length)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``filters by extension`` () =
        async {
            let! c = openTestDb ()
            let! _ = Db.upsertFiles c [ makeFile "a" "/p" "jpg"; makeFile "b" "/p" "png"; makeFile "c" "/p" "jpg" ]
            let! results = QueryEngine.executeSearch c { defaultQuery with Extensions = [ "jpg" ] }
            Assert.Equal(2, results.Length)
            Assert.True(results |> List.forall (fun f -> f.Ext = "jpg"))
        } |> Async.RunSynchronously

    [<Fact>]
    let ``filters by tag`` () =
        async {
            let! c = openTestDb ()
            let f1 = makeFile "tagged"   "/p" "jpg"
            let f2 = makeFile "untagged" "/p" "jpg"
            let! _ = Db.upsertFiles c [ f1; f2 ]
            do! Db.upsertTags c f1.Id [ "vacation" ]
            let! results = QueryEngine.executeSearch c { defaultQuery with Tags = [ "vacation" ] }
            Assert.Equal(1, results.Length)
            Assert.Equal(f1.Id, results[0].Id)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``filters by folder recursively`` () =
        async {
            let! c = openTestDb ()
            let root = makeFile "root" "/outer" "jpg"
            let nested = makeFile "nested" "/outer/inner" "jpg"
            let other = makeFile "other" "/other" "jpg"
            let! _ = Db.upsertFiles c [ root; nested; other ]
            let! results = QueryEngine.executeSearch c { defaultQuery with FolderPath = Some "/outer" }
            let ids = results |> List.map _.Id |> Set.ofList
            Assert.Equal(2, ids.Count)
            Assert.Contains(root.Id, ids)
            Assert.Contains(nested.Id, ids)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``folder filter does not match sibling with shared prefix`` () =
        async {
            let! c = openTestDb ()
            let nested = makeFile "nested" "/outer/inner" "jpg"
            let sibling = makeFile "sibling" "/outer-2" "jpg"
            let! _ = Db.upsertFiles c [ nested; sibling ]
            let! results = QueryEngine.executeSearch c { defaultQuery with FolderPath = Some "/outer" }
            Assert.Single(results) |> ignore
            Assert.Equal(nested.Id, results[0].Id)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``FTS matches by filename`` () =
        async {
            let! c = openTestDb ()
            let f1 = makeFile "beach_sunset"  "/p" "jpg"
            let f2 = makeFile "mountain_view" "/p" "jpg"
            let! _ = Db.upsertFiles c [ f1; f2 ]
            let! results = QueryEngine.executeSearch c { defaultQuery with Text = Some "beach" }
            Assert.Equal(1, results.Length)
            Assert.Equal(f1.Id, results[0].Id)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``FTS no match returns empty`` () =
        async {
            let! c = openTestDb ()
            let! _ = Db.upsertFiles c [ makeFile "sunset" "/p" "jpg" ]
            let! results = QueryEngine.executeSearch c { defaultQuery with Text = Some "xyznotfound" }
            Assert.Empty(results)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``excludes orphaned files`` () =
        async {
            let! c = openTestDb ()
            let f1 = makeFile "visible" "/p" "jpg"
            let f2 = makeFile "orphan"  "/p" "jpg"
            let! _ = Db.upsertFiles c [ f1; f2 ]
            do! Db.markOrphaned c f2.Id
            let! results = QueryEngine.executeSearch c defaultQuery
            Assert.Equal(1, results.Length)
            Assert.Equal(f1.Id, results[0].Id)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``sorts ascending by name`` () =
        async {
            let! c = openTestDb ()
            let! _ = Db.upsertFiles c [ makeFile "zebra" "/p" "jpg"; makeFile "alpha" "/p" "jpg"; makeFile "mango" "/p" "jpg" ]
            let! results = QueryEngine.executeSearch c { defaultQuery with SortBy = Name; SortAsc = true }
            let names = results |> List.map (fun f -> f.Name)
            Assert.Equal<string list>([ "alpha.jpg"; "mango.jpg"; "zebra.jpg" ], names)
        } |> Async.RunSynchronously
