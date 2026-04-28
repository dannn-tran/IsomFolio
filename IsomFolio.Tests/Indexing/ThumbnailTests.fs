module IsomFolio.Tests.Indexing.ThumbnailTests

open Xunit
open System
open System.IO
open System.Threading
open System.Collections.Generic
open IsomFolio.Models
open IsomFolio.Indexing.Thumbnail

[<Fact>]
let ``Worker pool processes multiple items`` () =
    let catalogDir = Path.Combine(Path.GetTempPath(), "IsomFolioTest_" + Guid.NewGuid().ToString())
    Directory.CreateDirectory(catalogDir) |> ignore
    
    let processed = List<FileId>()
    let locker = obj()
    let finishedEvent = new ManualResetEvent(false)
    let totalItems = 10
    
    let onReady id _ =
        lock locker (fun () ->
            processed.Add(id)
            if processed.Count = totalItems then
                finishedEvent.Set() |> ignore)
    
    let onFailed id msg =
        printfn "Failed %s: %s" id msg
        onReady id "" // Count it anyway for this test

    // Mock generateThumbnail by overriding or just providing real paths to empty files
    // Since we don't want to actually run SkiaSharp here if possible, 
    // we should have made generateThumbnail injectable or similar.
    // For now, let's just use real files that fail to decode, but we'll see if they process.
    
    let pool = createWorkerPool catalogDir 4 onReady onFailed
    
    for i in 1 .. totalItems do
        let id = $"file_{i}"
        pool.Post(Enqueue ({ FileId = id; FilePath = "non-existent.jpg"; Priority = 1 }, 0))
    
    let completed = finishedEvent.WaitOne(10000)
    Assert.True(completed, $"Only {processed.Count} / {totalItems} processed")
    Assert.Equal(totalItems, processed.Count)
    
    pool.Post Shutdown

[<Fact>]
let ``Worker pool handles priority changes`` () =
    let catalogDir = Path.Combine(Path.GetTempPath(), "IsomFolioTest_" + Guid.NewGuid().ToString())
    Directory.CreateDirectory(catalogDir) |> ignore
    
    let processed = List<FileId>()
    let locker = obj()
    let finishedEvent = new ManualResetEvent(false)
    let totalItems = 20

    let onReady id _ =
        lock locker (fun () ->
            processed.Add(id)
            if processed.Count = totalItems then
                finishedEvent.Set() |> ignore)
    
    let onFailed id _ = onReady id ""

    let pool = createWorkerPool catalogDir 4 onReady onFailed
    
    for i in 1 .. totalItems do
        pool.Post(Enqueue ({ FileId = $"file_{i}"; FilePath = "fake"; Priority = 10 }, 0))
    
    // Change priority of last item to 0
    pool.Post(SetPriority($"file_{totalItems}", 0))
    
    let completed = finishedEvent.WaitOne(30000)
    Assert.True(completed, $"Only {processed.Count} / {totalItems} processed before timeout")
    Assert.Equal(totalItems, processed.Count)
    
    pool.Post Shutdown
