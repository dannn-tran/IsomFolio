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
        do! Db.openDatabase dbPath
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
            do! openTestDb ()
            let! _ = Db.upsertFiles [ makeFile "alpha" "/photos" "jpg"; makeFile "beta" "/photos" "png" ]
            let! results = QueryEngine.executeSearch defaultQuery
            Assert.Equal(2, results.Length)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``filters by extension`` () =
        async {
            do! openTestDb ()
            let! _ = Db.upsertFiles [ makeFile "a" "/p" "jpg"; makeFile "b" "/p" "png"; makeFile "c" "/p" "jpg" ]
            let! results = QueryEngine.executeSearch { defaultQuery with Extensions = [ "jpg" ] }
            Assert.Equal(2, results.Length)
            Assert.True(results |> List.forall (fun f -> f.Ext = "jpg"))
        } |> Async.RunSynchronously

    [<Fact>]
    let ``filters by tag`` () =
        async {
            do! openTestDb ()
            let f1 = makeFile "tagged"   "/p" "jpg"
            let f2 = makeFile "untagged" "/p" "jpg"
            let! _ = Db.upsertFiles [ f1; f2 ]
            do! Db.upsertTags f1.Id [ "vacation" ]
            let! results = QueryEngine.executeSearch { defaultQuery with Tags = [ "vacation" ] }
            Assert.Equal(1, results.Length)
            Assert.Equal(f1.Id, results[0].Id)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``FTS matches by filename`` () =
        async {
            do! openTestDb ()
            let f1 = makeFile "beach_sunset"  "/p" "jpg"
            let f2 = makeFile "mountain_view" "/p" "jpg"
            let! _ = Db.upsertFiles [ f1; f2 ]
            let! results = QueryEngine.executeSearch { defaultQuery with Text = Some "beach" }
            Assert.Equal(1, results.Length)
            Assert.Equal(f1.Id, results[0].Id)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``FTS no match returns empty`` () =
        async {
            do! openTestDb ()
            let! _ = Db.upsertFiles [ makeFile "sunset" "/p" "jpg" ]
            let! results = QueryEngine.executeSearch { defaultQuery with Text = Some "xyznotfound" }
            Assert.Empty(results)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``excludes orphaned files`` () =
        async {
            do! openTestDb ()
            let f1 = makeFile "visible" "/p" "jpg"
            let f2 = makeFile "orphan"  "/p" "jpg"
            let! _ = Db.upsertFiles [ f1; f2 ]
            do! Db.markOrphaned f2.Id
            let! results = QueryEngine.executeSearch defaultQuery
            Assert.Equal(1, results.Length)
            Assert.Equal(f1.Id, results[0].Id)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``sorts ascending by name`` () =
        async {
            do! openTestDb ()
            let! _ = Db.upsertFiles [ makeFile "zebra" "/p" "jpg"; makeFile "alpha" "/p" "jpg"; makeFile "mango" "/p" "jpg" ]
            let! results = QueryEngine.executeSearch { defaultQuery with SortBy = Name; SortAsc = true }
            let names = results |> List.map (fun f -> f.Name)
            Assert.Equal<string list>([ "alpha.jpg"; "mango.jpg"; "zebra.jpg" ], names)
        } |> Async.RunSynchronously
