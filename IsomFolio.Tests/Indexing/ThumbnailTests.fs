module IsomFolio.Tests.Indexing.ThumbnailTests

open Xunit
open System
open System.IO
open System.Threading
open System.Collections.Generic
open SkiaSharp
open IsomFolio.Models
open IsomFolio.Indexing.Thumbnail

let private writePng (path: string) =
    use bmp = new SKBitmap(8, 8)
    use canvas = new SKCanvas(bmp)
    canvas.Clear(SKColors.Red)
    use img = SKImage.FromBitmap(bmp)
    use data = img.Encode(SKEncodedImageFormat.Png, 100)
    use fs = File.Create(path)
    data.SaveTo(fs)

let private makeReq (catalogDir: string) (idx: int) =
    let imgPath = Path.Combine(catalogDir, $"src_{idx}.png")
    writePng imgPath
    { FileId = $"file_{idx}"; FilePath = imgPath; Priority = 1 }

let private waitFor (event: ManualResetEvent) (timeoutMs: int) =
    event.WaitOne(timeoutMs)

module WorkerPool =

    [<Fact>]
    let ``processes multiple items in a single batch`` () =
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
            onReady id ""

        let pool = createWorkerPool catalogDir 4 onReady onFailed

        for i in 1 .. totalItems do
            let id = $"file_{i}"
            pool.Post(Enqueue ({ FileId = id; FilePath = "non-existent.jpg"; Priority = 1 }, 0))

        let completed = waitFor finishedEvent 10000
        Assert.True(completed, $"Only {processed.Count} / {totalItems} processed")
        Assert.Equal(totalItems, processed.Count)

        pool.Post Shutdown

    [<Fact>]
    let ``processes a second batch posted after the first completes`` () =
        let catalogDir = Path.Combine(Path.GetTempPath(), "IsomFolioTest_" + Guid.NewGuid().ToString())
        Directory.CreateDirectory(catalogDir) |> ignore

        let mutable expected = 0
        let processed = List<FileId>()
        let locker = obj()
        let mutable doneEvent = new ManualResetEvent(false)

        let onReady id _ =
            lock locker (fun () ->
                processed.Add(id)
                if processed.Count = expected then doneEvent.Set() |> ignore)

        let onFailed id msg =
            printfn "Failed %s: %s" id msg
            onReady id ""

        let pool = createWorkerPool catalogDir 4 onReady onFailed

        // Batch 1
        let batch1 = [ for i in 1 .. 5 -> makeReq catalogDir i ]
        expected <- 5
        for r in batch1 do pool.Post(Enqueue (r, 0))
        Assert.True(waitFor doneEvent 10000, $"batch 1 did not finish: {processed.Count}/5")

        // Batch 2 — same pool, after batch 1 fully drained
        doneEvent <- new ManualResetEvent(false)
        let batch2 = [ for i in 6 .. 10 -> makeReq catalogDir i ]
        expected <- 10
        for r in batch2 do pool.Post(Enqueue (r, 0))
        Assert.True(waitFor doneEvent 10000, $"batch 2 did not finish: {processed.Count}/10")

        Assert.Equal(10, processed.Count)
        pool.Post Shutdown
