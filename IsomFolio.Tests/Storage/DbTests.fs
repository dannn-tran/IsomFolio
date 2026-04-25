module IsomFolio.Tests.Storage.DbTests

open System
open System.IO
open Xunit
open IsomFolio.Models
open IsomFolio.Storage
open IsomFolio.Indexing

// -------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------

/// Open a fresh on-disk DB per test (isolated temp file)
let private openTestDb () =
    async {
        let dbPath = Path.Combine(Path.GetTempPath(), $"isomfolio_test_{Guid.NewGuid():N}.db")
        do! Db.openDatabase dbPath
        return dbPath
    }

let private sampleFile (n: int) : AssetFile =
    let path = $"/photos/test{n}.jpg"
    {
        Id         = IsomFolio.FileIndex.computeFileId path
        Path       = path
        Name       = $"test{n}.jpg"
        Folder     = "/photos"
        Ext        = "jpg"
        SizeBytes  = int64 (n * 1024)
        MTimeUnix  = DateTimeOffset.UtcNow.ToUnixTimeSeconds()
        IsOrphaned = false
        OrphanedAt = None
    }

// -------------------------------------------------------------------------
// DB tests
// -------------------------------------------------------------------------

[<Fact>]
let ``upsertFiles inserts and retrieves by folder`` () =
    async {
        let! _ = openTestDb ()
        let files = [ sampleFile 1; sampleFile 2; sampleFile 3 ]
        let! inserted = Db.upsertFiles files
        Assert.Equal(3, inserted)
        let! retrieved = Db.getFilesByFolder "/photos"
        Assert.Equal(3, retrieved.Length)
    } |> Async.RunSynchronously

[<Fact>]
let ``upsertFiles is idempotent`` () =
    async {
        let! _ = openTestDb ()
        let files = [ sampleFile 1 ]
        let! _ = Db.upsertFiles files
        let! _ = Db.upsertFiles files
        let! retrieved = Db.getFilesByFolder "/photos"
        Assert.Equal(1, retrieved.Length)
    } |> Async.RunSynchronously

[<Fact>]
let ``markOrphaned excludes file from folder query`` () =
    async {
        let! _ = openTestDb ()
        let f = sampleFile 1
        let! _ = Db.upsertFiles [ f ]
        do! Db.markOrphaned f.Id
        let! retrieved = Db.getFilesByFolder "/photos"
        Assert.Empty(retrieved)
        let! byId = Db.getFileById f.Id
        let found = Option.get byId
        Assert.True(found.IsOrphaned)
        Assert.True(found.OrphanedAt.IsSome)
    } |> Async.RunSynchronously

[<Fact>]
let ``upsertTags and getTagsForFile round-trip`` () =
    async {
        let! _ = openTestDb ()
        let f = sampleFile 1
        let! _ = Db.upsertFiles [ f ]
        do! Db.upsertTags f.Id [ "vacation"; "beach"; "2024" ]
        let! tags = Db.getTagsForFile f.Id
        Assert.Equal<string list>([ "2024"; "beach"; "vacation" ], tags)
    } |> Async.RunSynchronously

[<Fact>]
let ``upsertTags replaces previous tags`` () =
    async {
        let! _ = openTestDb ()
        let f = sampleFile 1
        let! _ = Db.upsertFiles [ f ]
        do! Db.upsertTags f.Id [ "old"; "tags" ]
        do! Db.upsertTags f.Id [ "new" ]
        let! tags = Db.getTagsForFile f.Id
        Assert.Equal<string list>([ "new" ], tags)
    } |> Async.RunSynchronously

[<Fact>]
let ``getAllTags returns usage counts`` () =
    async {
        let! _ = openTestDb ()
        let f1 = sampleFile 1
        let f2 = sampleFile 2
        let! _ = Db.upsertFiles [ f1; f2 ]
        do! Db.upsertTags f1.Id [ "shared"; "unique1" ]
        do! Db.upsertTags f2.Id [ "shared"; "unique2" ]
        let! allTags = Db.getAllTags ()
        let tagMap = allTags |> Map.ofList
        Assert.Equal(2, tagMap["shared"])
        Assert.Equal(1, tagMap["unique1"])
        Assert.Equal(1, tagMap["unique2"])
    } |> Async.RunSynchronously

[<Fact>]
let ``purgeOldOrphans removes stale records`` () =
    async {
        let! _ = openTestDb ()
        let f = sampleFile 1
        let! _ = Db.upsertFiles [ f ]
        do! Db.markOrphaned f.Id
        do! Db.executeRaw $"UPDATE files SET orphaned_at = {DateTimeOffset.UtcNow.AddDays(-40.0).ToUnixTimeSeconds()} WHERE id = '{f.Id}'"
        let! purged = Db.purgeOldOrphans 30
        Assert.Equal(1, purged)
        let! byId = Db.getFileById f.Id
        Assert.True(byId.IsNone)
    } |> Async.RunSynchronously

// -------------------------------------------------------------------------
// Scanner tests
// -------------------------------------------------------------------------

let private createTempImageFiles (count: int) =
    let dir = Path.Combine(Path.GetTempPath(), $"isomfolio_scan_{Guid.NewGuid():N}")
    Directory.CreateDirectory(dir) |> ignore
    for i in 1..count do
        // Minimal valid 1x1 JPEG so FileInfo works; content doesn't matter for scanner
        File.WriteAllBytes(Path.Combine(dir, $"img{i}.jpg"), [| 0xFFuy; 0xD8uy; 0xFFuy; 0xD9uy |])
    // Also create a non-image file that should be ignored
    File.WriteAllText(Path.Combine(dir, "notes.txt"), "ignored")
    dir

[<Fact>]
let ``scanFolder indexes supported files and ignores unsupported`` () =
    async {
        let! _ = openTestDb ()
        let dir = createTempImageFiles 5
        try
            let mutable progressCalls = 0
            let! result =
                Scanner.scanFolder
                    dir
                    (fun batch -> async { let! _ = Db.upsertFiles batch in () })
                    (fun _ -> progressCalls <- progressCalls + 1)
            Assert.Equal(5, result.TotalCount)
            Assert.True(progressCalls >= 1)
            let! inDb = Db.getFilesByFolder dir
            Assert.Equal(5, inDb.Length)
        finally
            Directory.Delete(dir, true)
    } |> Async.RunSynchronously

[<Fact>]
let ``reconcileFolder detects new files and orphans`` () =
    async {
        let! _ = openTestDb ()
        let dir = createTempImageFiles 3
        try
            // Index initial state
            let! _ =
                Scanner.scanFolder dir
                    (fun batch -> async { let! _ = Db.upsertFiles batch in () })
                    ignore
            // Add a new file
            File.WriteAllBytes(Path.Combine(dir, "new.png"), [| 0x89uy; 0x50uy |])
            // Delete an existing file
            let toDelete = Directory.GetFiles(dir, "*.jpg")[0]
            File.Delete(toDelete)

            let! newOrModified, orphaned = Scanner.reconcileFolder dir
            Assert.Equal(1, newOrModified.Length)   // new.png
            Assert.Equal(1, orphaned.Length)         // deleted jpg
        finally
            Directory.Delete(dir, true)
    } |> Async.RunSynchronously
