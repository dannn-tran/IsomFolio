module IsomFolio.Core.Tests.Storage.DbTests

open System
open System.IO
open Xunit
open IsomFolio.Core.Models
open IsomFolio.Core.Metadata
open IsomFolio.Core.Metadata.Xmp
open IsomFolio.Core.Storage
open IsomFolio.Core.Search
open IsomFolio.Core.Indexing

let private openTestDb () =
    async {
        let dbPath = Path.Combine(Path.GetTempPath(), $"isomfolio_test_{Guid.NewGuid():N}.db")
        return! Db.openDatabase dbPath
    }

let private sampleFile (n: int) : AssetFile =
    let path = $"/photos/test{n}.jpg"
    {
        Id            = IsomFolio.Core.FileIndex.computeFileId path
        Path          = path
        Name          = $"test{n}.jpg"
        Folder        = "/photos"
        Ext           = "jpg"
        SizeBytes     = int64 (n * 1024)
        MTimeUnix     = DateTimeOffset.UtcNow.ToUnixTimeSeconds()
        CreatedAtUnix = DateTimeOffset.UtcNow.ToUnixTimeSeconds()
        IsOrphaned    = false
        OrphanedAt    = None
    }

let private createTempImageDir (count: int) =
    let dir = Path.Combine(Path.GetTempPath(), $"isomfolio_scan_{Guid.NewGuid():N}")
    Directory.CreateDirectory(dir) |> ignore
    for i in 1..count do
        File.WriteAllBytes(Path.Combine(dir, $"img{i}.jpg"), [| 0xFFuy; 0xD8uy; 0xFFuy; 0xD9uy |])
    File.WriteAllText(Path.Combine(dir, "notes.txt"), "ignored")
    dir


module UpsertFiles =

    [<Fact>]
    let ``inserts and retrieves by folder`` () =
        async {
            let! c = openTestDb ()
            let! inserted = [ sampleFile 1; sampleFile 2; sampleFile 3 ] |> Db.upsertFiles c
            Assert.Equal(3, inserted)
            let! retrieved = "/photos" |> Db.getFilesByFolder c
            Assert.Equal(3, retrieved.Length)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``is idempotent`` () =
        async {
            let! c = openTestDb ()
            let! _ = Db.upsertFiles c [ sampleFile 1 ]
            let! _ = Db.upsertFiles c [ sampleFile 1 ]
            let! retrieved = Db.getFilesByFolder c "/photos"
            Assert.Equal(1, retrieved.Length)
        } |> Async.RunSynchronously


module MarkOrphaned =

    [<Fact>]
    let ``excludes file from folder query`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.markOrphaned c f.Id
            let! retrieved = Db.getFilesByFolder c "/photos"
            Assert.Empty(retrieved)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``sets is_orphaned and orphaned_at`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.markOrphaned c f.Id
            let! byId = Db.getFileById c f.Id
            let found = Option.get byId
            Assert.True(found.IsOrphaned)
            Assert.True(found.OrphanedAt.IsSome)
        } |> Async.RunSynchronously


module UpsertTags =

    [<Fact>]
    let ``round-trips tag list`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertTags c f.Id [ "vacation"; "beach"; "2024" ]
            let! tags = Db.getTagsForFile c f.Id
            Assert.Equal<string list>([ "2024"; "beach"; "vacation" ], tags)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``replaces previous tags`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertTags c f.Id [ "old"; "tags" ]
            do! Db.upsertTags c f.Id [ "new" ]
            let! tags = Db.getTagsForFile c f.Id
            Assert.Equal<string list>([ "new" ], tags)
        } |> Async.RunSynchronously


module GetAllTags =

    [<Fact>]
    let ``returns usage counts across files`` () =
        async {
            let! c = openTestDb ()
            let f1 = sampleFile 1
            let f2 = sampleFile 2
            let! _ = Db.upsertFiles c [ f1; f2 ]
            do! Db.upsertTags c f1.Id [ "shared"; "unique1" ]
            do! Db.upsertTags c f2.Id [ "shared"; "unique2" ]
            let! allTags = Db.getAllTags c
            let tagMap = allTags |> Map.ofList
            Assert.Equal(2, tagMap["shared"])
            Assert.Equal(1, tagMap["unique1"])
            Assert.Equal(1, tagMap["unique2"])
        } |> Async.RunSynchronously


module PurgeOldOrphans =

    [<Fact>]
    let ``removes records orphaned beyond threshold`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.markOrphaned c f.Id
            do! Db.executeRaw c $"UPDATE files SET orphaned_at = {DateTimeOffset.UtcNow.AddDays(-40.0).ToUnixTimeSeconds()} WHERE id = '{f.Id}'"
            let! purged = Db.purgeOldOrphans c 30
            Assert.Equal(1, purged)
            let! byId = Db.getFileById c f.Id
            Assert.True(byId.IsNone)
        } |> Async.RunSynchronously


module ScanFolder =

    [<Fact>]
    let ``indexes supported files and ignores unsupported`` () =
        async {
            let! c = openTestDb ()
            let dir = createTempImageDir 5
            try
                let mutable progressCalls = 0
                let! result =
                    Scanner.scanFolder
                        dir
                        (fun batch -> async {
                            let assets = batch |> List.map (fun sf -> sf.Asset)
                            let! _ = Db.upsertFiles c assets
                            ()
                        })
                        (fun _ -> progressCalls <- progressCalls + 1)
                Assert.Equal(5, result.TotalCount)
                Assert.True(progressCalls >= 1)
                let! inDb = Db.getFilesByFolder c dir
                Assert.Equal(5, inDb.Length)
            finally
                Directory.Delete(dir, true)
        } |> Async.RunSynchronously


module ReconcileFolder =

    [<Fact>]
    let ``detects new files and orphans`` () =
        async {
            let! c = openTestDb ()
            let dir = createTempImageDir 3
            try
                let! _ =
                    Scanner.scanFolder dir
                        (fun batch -> async {
                            let! _ = Db.upsertFiles c (batch |> List.map (fun sf -> sf.Asset))
                            ()
                        })
                        ignore
                File.WriteAllBytes(Path.Combine(dir, "new.png"), [| 0x89uy; 0x50uy |])
                File.Delete(Directory.GetFiles(dir, "*.jpg")[0])

                let! result = Scanner.reconcileFolder c dir
                Assert.Equal(1, result.NewOrModified.Length)
                Assert.Equal(1, result.Orphaned.Length)
            finally
                Directory.Delete(dir, true)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``detects sidecar newer than indexed image`` () =
        async {
            let! c = openTestDb ()
            let dir = Path.Combine(Path.GetTempPath(), $"isomfolio_scan_{Guid.NewGuid():N}")
            Directory.CreateDirectory(dir) |> ignore
            let imgPath = Path.Combine(dir, "photo.jpg")
            File.WriteAllBytes(imgPath, [| 0xFFuy; 0xD8uy; 0xFFuy; 0xD9uy |])
            try
                let! _ =
                    Scanner.scanFolder dir
                        (fun batch -> async {
                            let! _ = Db.upsertFiles c (batch |> List.map (fun sf -> sf.Asset))
                            ()
                        })
                        ignore
                let sidecarPath = Path.Combine(dir, "photo.xmp")
                File.WriteAllText(sidecarPath, "<x:xmpmeta/>")
                File.SetLastWriteTimeUtc(sidecarPath, File.GetLastWriteTimeUtc(imgPath).AddSeconds(2.0))

                let! result = Scanner.reconcileFolder c dir
                let normalize = IsomFolio.Core.PathUtils.normalizePath
                Assert.Contains(
                    normalize imgPath,
                    result.SidecarChanged |> List.map normalize)
            finally
                Directory.Delete(dir, true)
        } |> Async.RunSynchronously


module UpsertMetadata =

    let private makeMetaWithRating (rating: int) : EmbeddedMetadata =
        {
            Xmp = Some {
                Core = { XmpCore.empty with Rating = Some rating }
                DublinCore = { DublinCore.empty with Subject = [ "nature"; "travel" ] }
            }
            AppleMetadata = None
        }

    [<Fact>]
    let ``persists rating and subjects`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertMetadata c f.Id (makeMetaWithRating 3)
            use cmd = c.CreateCommand()
            cmd.CommandText <- "SELECT rating, subjects FROM metadata WHERE file_id = @id"
            cmd.Parameters.AddWithValue("@id", f.Id) |> ignore
            use reader = cmd.ExecuteReader()
            Assert.True(reader.Read())
            Assert.Equal(3, reader.GetInt32(0))
            let subjects = System.Text.Json.JsonSerializer.Deserialize<string list>(reader.GetString(1))
            Assert.Equivalent([ "nature"; "travel" ], subjects)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``upsert is idempotent`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertMetadata c f.Id (makeMetaWithRating 4)
            do! Db.upsertMetadata c f.Id (makeMetaWithRating 5)
            use cmd = c.CreateCommand()
            cmd.CommandText <- "SELECT COUNT(*) FROM metadata WHERE file_id = @id"
            cmd.Parameters.AddWithValue("@id", f.Id) |> ignore
            let count = cmd.ExecuteScalar() :?> int64
            Assert.Equal(1L, count)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``subjects appear in FTS index`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertMetadata c f.Id (makeMetaWithRating 3)
            let! ids = FTS.searchFts5 c "nature"
            Assert.Contains(f.Id, ids)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``getMetadata round-trips stored rating and subjects`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertMetadata c f.Id (makeMetaWithRating 4)
            let! result = Db.getMetadata c f.Id
            match result with
            | Some meta ->
                Assert.Equal(Some 4, meta.Xmp |> Option.bind (fun x -> x.Core.Rating))
                let subjects = meta.Xmp |> Option.map (fun x -> x.DublinCore.Subject) |> Option.defaultValue []
                Assert.Equivalent([ "nature"; "travel" ], subjects)
            | None -> Assert.Fail("Expected Some metadata")
        } |> Async.RunSynchronously


module RenameTag =

    [<Fact>]
    let ``exact rename updates matching rows`` () =
        async {
            let! c = openTestDb ()
            let f1 = sampleFile 1
            let f2 = sampleFile 2
            let! _ = Db.upsertFiles c [ f1; f2 ]
            do! Db.upsertTags c f1.Id [ "beach"; "travel" ]
            do! Db.upsertTags c f2.Id [ "beach" ]
            let! count = Db.renameTag c "beach" "seaside"
            Assert.Equal(2, count)
            let! tags1 = Db.getTagsForFile c f1.Id
            Assert.Equal<string list>([ "seaside"; "travel" ], tags1 |> List.sort)
            let! tags2 = Db.getTagsForFile c f2.Id
            Assert.Equal<string list>([ "seaside" ], tags2)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``no-op when tag not found`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertTags c f.Id [ "travel" ]
            let! count = Db.renameTag c "beach" "seaside"
            Assert.Equal(0, count)
            let! tags = Db.getTagsForFile c f.Id
            Assert.Equal<string list>([ "travel" ], tags)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``prefix rename updates exact tag and all descendants`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertTags c f.Id [ "person"; "person/John"; "person/Jane"; "place" ]
            let! count = Db.renamePrefixedTags c "person" "people"
            Assert.Equal(3, count)
            let! tags = Db.getTagsForFile c f.Id
            Assert.Equal<string list>([ "people"; "people/Jane"; "people/John"; "place" ], tags |> List.sort)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``prefix rename does not affect unrelated tags`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertTags c f.Id [ "personal"; "person/John" ]
            let! _ = Db.renamePrefixedTags c "person" "people"
            let! tags = Db.getTagsForFile c f.Id
            Assert.Contains("personal", tags)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``prefix rename no-op when prefix not found`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertTags c f.Id [ "travel" ]
            let! count = Db.renamePrefixedTags c "person" "people"
            Assert.Equal(0, count)
        } |> Async.RunSynchronously


module DeleteTag =

    [<Fact>]
    let ``deletes exact tag from all files`` () =
        async {
            let! c = openTestDb ()
            let f1 = sampleFile 1
            let f2 = sampleFile 2
            let! _ = Db.upsertFiles c [ f1; f2 ]
            do! Db.upsertTags c f1.Id [ "beach"; "travel" ]
            do! Db.upsertTags c f2.Id [ "beach" ]
            let! count = Db.deleteTagWithDescendants c "beach"
            Assert.Equal(2, count)
            let! tags1 = Db.getTagsForFile c f1.Id
            Assert.Equal<string list>([ "travel" ], tags1)
            let! tags2 = Db.getTagsForFile c f2.Id
            Assert.Empty(tags2)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``deletes tag and all descendants`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertTags c f.Id [ "person"; "person/John"; "person/Jane"; "place" ]
            let! count = Db.deleteTagWithDescendants c "person"
            Assert.Equal(3, count)
            let! tags = Db.getTagsForFile c f.Id
            Assert.Equal<string list>([ "place" ], tags)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``does not delete unrelated tags with similar prefix`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertTags c f.Id [ "personal"; "person/John" ]
            let! _ = Db.deleteTagWithDescendants c "person"
            let! tags = Db.getTagsForFile c f.Id
            Assert.Contains("personal", tags)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``no-op when tag not found`` () =
        async {
            let! c = openTestDb ()
            let f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            do! Db.upsertTags c f.Id [ "travel" ]
            let! count = Db.deleteTagWithDescendants c "beach"
            Assert.Equal(0, count)
        } |> Async.RunSynchronously

module SearchQuerySerialization =

    let private defaultQuery : IsomFolio.Core.Models.SearchQuery = {
        Text       = None
        FolderPath = None
        Tags       = []
        Extensions = []
        DateRange  = None
        SortBy     = Date
        SortAsc    = false
    }

    [<Fact>]
    let ``round-trips empty query`` () =
        let json = Db.serializeSearchQuery defaultQuery
        let q    = Db.deserializeSearchQuery json
        Assert.Equal(defaultQuery, q)

    [<Fact>]
    let ``round-trips full query`` () =
        let from = DateTime(2024, 1, 1)
        let until = DateTime(2024, 12, 31)
        let q = {
            defaultQuery with
                Text       = Some "canal"
                FolderPath = Some "/photos/2024"
                Tags       = [ "paris"; "architecture" ]
                Extensions = [ "jpg"; "png" ]
                DateRange  = Some(from, until)
                SortBy     = Name
                SortAsc    = true
        }
        let q2 = q |> Db.serializeSearchQuery |> Db.deserializeSearchQuery
        Assert.Equal(q.Text,       q2.Text)
        Assert.Equal(q.FolderPath, q2.FolderPath)
        Assert.Equal<string list>(q.Tags,       q2.Tags)
        Assert.Equal<string list>(q.Extensions, q2.Extensions)
        Assert.Equal(q.SortBy,    q2.SortBy)
        Assert.Equal(q.SortAsc,   q2.SortAsc)
        Assert.True(q2.DateRange.IsSome)
        let (f2, t2) = q2.DateRange.Value
        Assert.Equal(from.Date,  f2.Date)
        Assert.Equal(until.Date, t2.Date)

module Albums =

    let private newAlbum name kind = {
        Id        = System.Guid.NewGuid().ToString("N")
        Name      = name
        Kind      = kind
        SortOrder = 0
    }

    [<Fact>]
    let ``create and retrieve manual album`` () =
        async {
            let! c     = openTestDb ()
            let  album = newAlbum "Picks" Manual
            do!  Db.createAlbum c album
            let! all   = Db.getAllAlbums c
            Assert.Equal(1, all.Length)
            Assert.Equal("Picks", all.[0].Name)
            Assert.Equal(Manual,  all.[0].Kind)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``create and retrieve smart album preserves query`` () =
        async {
            let! c   = openTestDb ()
            let  q   = { Text = Some "paris"; FolderPath = None; Tags = [ "travel" ]; Extensions = []; DateRange = None; SortBy = Date; SortAsc = false }
            let  album = newAlbum "Paris" (Smart q)
            do!  Db.createAlbum c album
            let! all = Db.getAllAlbums c
            match all.[0].Kind with
            | Smart q2 ->
                Assert.Equal(q.Text, q2.Text)
                Assert.Equal<string list>(q.Tags, q2.Tags)
            | Manual -> Assert.Fail("Expected Smart album")
        } |> Async.RunSynchronously

    [<Fact>]
    let ``rename album`` () =
        async {
            let! c = openTestDb ()
            let a  = newAlbum "Old" Manual
            do!  Db.createAlbum c a
            do!  Db.renameAlbum c a.Id "New"
            let! all = Db.getAllAlbums c
            Assert.Equal("New", all.[0].Name)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``delete album removes it`` () =
        async {
            let! c = openTestDb ()
            let a  = newAlbum "Gone" Manual
            do!  Db.createAlbum c a
            do!  Db.deleteAlbum c a.Id
            let! all = Db.getAllAlbums c
            Assert.Equal(0, all.Length)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``update smart album query`` () =
        async {
            let! c = openTestDb ()
            let  q1 = { Text = Some "old"; FolderPath = None; Tags = []; Extensions = []; DateRange = None; SortBy = Date; SortAsc = false }
            let  q2 = { q1 with Text = Some "new" }
            let  a  = newAlbum "Smart" (Smart q1)
            do!  Db.createAlbum c a
            do!  Db.updateSmartAlbumQuery c a.Id q2
            let! all = Db.getAllAlbums c
            match all.[0].Kind with
            | Smart q -> Assert.Equal(Some "new", q.Text)
            | Manual  -> Assert.Fail("Expected Smart")
        } |> Async.RunSynchronously

    [<Fact>]
    let ``add and remove file from album`` () =
        async {
            let! c = openTestDb ()
            let  f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            let  a = newAlbum "Manual" Manual
            do!  Db.createAlbum c a
            do!  Db.addFileToAlbum c a.Id f.Id
            let! n = Db.countAlbumFiles c a.Id
            Assert.Equal(1, n)
            do!  Db.removeFileFromAlbum c a.Id f.Id
            let! n2 = Db.countAlbumFiles c a.Id
            Assert.Equal(0, n2)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``add file to album is idempotent`` () =
        async {
            let! c = openTestDb ()
            let  f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            let  a = newAlbum "Manual" Manual
            do!  Db.createAlbum c a
            do!  Db.addFileToAlbum c a.Id f.Id
            do!  Db.addFileToAlbum c a.Id f.Id
            let! n = Db.countAlbumFiles c a.Id
            Assert.Equal(1, n)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``deleting a file cascades to album_files`` () =
        async {
            let! c = openTestDb ()
            let  f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            let  a = newAlbum "Manual" Manual
            do!  Db.createAlbum c a
            do!  Db.addFileToAlbum c a.Id f.Id
            do!  Db.deleteFile c f.Id
            let! n = Db.countAlbumFiles c a.Id
            Assert.Equal(0, n)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``deleting album cascades to album_files`` () =
        async {
            let! c = openTestDb ()
            let  f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            let  a = newAlbum "Manual" Manual
            do!  Db.createAlbum c a
            do!  Db.addFileToAlbum c a.Id f.Id
            do!  Db.deleteAlbum c a.Id
            let! all = Db.getAllAlbums c
            Assert.Equal(0, all.Length)
        } |> Async.RunSynchronously

module ManualAlbumSearch =

    [<Fact>]
    let ``returns files in album ordered by insertion time`` () =
        async {
            let! c  = openTestDb ()
            let  f1 = sampleFile 1
            let  f2 = sampleFile 2
            let! _  = Db.upsertFiles c [ f1; f2 ]
            let  a  = { Id = System.Guid.NewGuid().ToString("N"); Name = "A"; Kind = Manual; SortOrder = 0 }
            do!  Db.createAlbum c a
            do!  Db.addFileToAlbum c a.Id f1.Id
            do!  System.Threading.Tasks.Task.Delay(10) |> Async.AwaitTask
            do!  Db.addFileToAlbum c a.Id f2.Id
            let! results = IsomFolio.Core.Search.QueryEngine.executeManualAlbumSearch c a.Id
            Assert.Equal(2, results.Length)
            Assert.Equal(f1.Id, results.[0].Id)
            Assert.Equal(f2.Id, results.[1].Id)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``returns empty list for empty album`` () =
        async {
            let! c = openTestDb ()
            let  a = { Id = System.Guid.NewGuid().ToString("N"); Name = "Empty"; Kind = Manual; SortOrder = 0 }
            do!  Db.createAlbum c a
            let! results = IsomFolio.Core.Search.QueryEngine.executeManualAlbumSearch c a.Id
            Assert.Equal(0, results.Length)
        } |> Async.RunSynchronously

    [<Fact>]
    let ``includes orphaned files`` () =
        async {
            let! c = openTestDb ()
            let  f = sampleFile 1
            let! _ = Db.upsertFiles c [ f ]
            let  a = { Id = System.Guid.NewGuid().ToString("N"); Name = "A"; Kind = Manual; SortOrder = 0 }
            do!  Db.createAlbum c a
            do!  Db.addFileToAlbum c a.Id f.Id
            do!  Db.markOrphaned c f.Id
            let! results = IsomFolio.Core.Search.QueryEngine.executeManualAlbumSearch c a.Id
            Assert.Equal(1, results.Length)
            Assert.True(results.[0].IsOrphaned)
        } |> Async.RunSynchronously
