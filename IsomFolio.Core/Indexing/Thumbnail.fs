module IsomFolio.Indexing.Thumbnail

open System
open System.IO
open System.Collections.Generic
open Microsoft.Data.Sqlite
open SkiaSharp
open IsomFolio.Models
open IsomFolio.AppPaths

// ---------------------------------------------------------------------------
// Cache helpers
// ---------------------------------------------------------------------------

let thumbnailCachePath (catalogDir: string) (fileId: FileId) : string =
    Path.Combine(thumbnailCacheDir catalogDir, fileId + ".jpg")

let isCacheValid (catalogDir: string) (fileId: FileId) : bool =
    File.Exists(thumbnailCachePath catalogDir fileId)

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

let private targetSize = 256

/// Decode, resize to 256px longest edge, encode as JPEG 85, atomic write.
/// Returns Ok(cachePath) or Error(message).
let generateThumbnail (catalogDir: string) (req: ThumbnailRequest) : Async<Result<string, string>> =
    async {
        let dest = thumbnailCachePath catalogDir req.FileId
        if File.Exists(dest) then
            return Ok dest
        else
            try
                ensureDirectories catalogDir

                use bitmap = SKBitmap.Decode(req.FilePath)
                if isNull bitmap then
                    return Error $"SKBitmap.Decode returned null for {req.FilePath}"
                else
                    // Scale so longest edge = targetSize, preserve aspect ratio
                    let w, h = bitmap.Width, bitmap.Height
                    let scale = float targetSize / float (max w h)
                    let newW = max 1 (int (float w * scale))
                    let newH = max 1 (int (float h * scale))

                    use scaled = bitmap.Resize(SKImageInfo(newW, newH), SKFilterQuality.Medium)
                    if isNull scaled then
                        return Error $"SKBitmap.Resize failed for {req.FilePath}"
                    else
                        use image  = SKImage.FromBitmap(scaled)
                        use data   = image.Encode(SKEncodedImageFormat.Jpeg, 85)

                        let tmp = dest + ".tmp"
                        use fs = File.OpenWrite(tmp)
                        data.SaveTo(fs)
                        fs.Flush()
                        File.Move(tmp, dest, overwrite = true)
                        return Ok dest
            with ex ->
                return Error ex.Message
    }

// ---------------------------------------------------------------------------
// Worker pool
// ---------------------------------------------------------------------------

type ThumbnailMsg =
    | Enqueue   of ThumbnailRequest
    | SetPriority of fileId: FileId * priority: int
    | CancelAll
    | Shutdown

/// Creates a background worker pool backed by a MailboxProcessor priority queue.
/// concurrency: number of parallel workers (default 4).
/// onReady / onFailed: callbacks invoked on the calling context (route through Dispatcher.UIThread.Post in App layer).
let createWorkerPool
    (catalogDir   : string)
    (concurrency  : int)
    (onReady  : FileId -> string -> unit)
    (onFailed : FileId -> string -> unit)
    : MailboxProcessor<ThumbnailMsg> =

    // Priority queue: SortedDictionary<priority, Queue<ThumbnailRequest>>
    // Lower priority int = higher urgency (0 = visible tile)
    let queue   = SortedDictionary<int, Queue<ThumbnailRequest>>()
    let inFlight = System.Collections.Concurrent.ConcurrentDictionary<FileId, bool>()
    let sem     = new System.Threading.SemaphoreSlim(concurrency, concurrency)

    let enqueueItem (req: ThumbnailRequest) =
        if not (inFlight.ContainsKey(req.FileId)) then
            if not (queue.ContainsKey(req.Priority)) then
                queue[req.Priority] <- Queue<ThumbnailRequest>()
            queue[req.Priority].Enqueue(req)

    let dequeueItem () =
        let result =
            queue
            |> Seq.tryFind (fun kv -> kv.Value.Count > 0)
            |> Option.map (fun kv -> kv.Value.Dequeue())
        // Prune empty buckets
        queue |> Seq.filter (fun kv -> kv.Value.Count = 0)
              |> Seq.map (fun kv -> kv.Key)
              |> Seq.toList
              |> List.iter (queue.Remove >> ignore)
        result

    let agent = MailboxProcessor.Start(fun inbox ->
        let rec loop () =
            async {
                let! msg = inbox.Receive()
                match msg with
                | Shutdown -> ()
                | CancelAll ->
                    queue.Clear()
                    return! loop ()
                | SetPriority(fileId, priority) ->
                    // Re-enqueue with new priority — remove from old bucket first
                    for kv in queue do
                        let newQ = Queue(kv.Value |> Seq.filter (fun r -> r.FileId <> fileId))
                        kv.Value.Clear()
                        for r in newQ do kv.Value.Enqueue(r)
                    enqueueItem { FileId = fileId; FilePath = ""; Priority = priority }
                    return! loop ()
                | Enqueue req ->
                    enqueueItem req
                    // Dispatch a worker if semaphore permits
                    match dequeueItem () with
                    | None -> ()
                    | Some work ->
                        inFlight[work.FileId] <- true
                        do! sem.WaitAsync() |> Async.AwaitTask
                        Async.Start(async {
                            try
                                let! result = generateThumbnail catalogDir work
                                match result with
                                | Ok path  -> onReady  work.FileId path
                                | Error msg ->
                                    // Retry once after 5 seconds
                                    do! Async.Sleep 5000
                                    let! retry = generateThumbnail catalogDir work
                                    match retry with
                                    | Ok path -> onReady  work.FileId path
                                    | Error _ -> onFailed work.FileId msg
                            finally
                                inFlight.TryRemove(work.FileId) |> ignore
                                sem.Release() |> ignore
                        })
                    return! loop ()
            }
        loop ())

    agent

// ---------------------------------------------------------------------------
// Cache maintenance
// ---------------------------------------------------------------------------

/// Remove thumbnails from the cache dir that have no matching file in the DB.
/// Called at startup after the initial scan completes.
let sweepThumbnailCache (c: SqliteConnection) (catalogDir: string) : Async<int> =
    async {
        let cacheDir = thumbnailCacheDir catalogDir
        if not (Directory.Exists(cacheDir)) then return 0
        else
            let mutable removed = 0
            for file in Directory.EnumerateFiles(cacheDir, "*.jpg") do
                let fileId = Path.GetFileNameWithoutExtension(file)
                let! existing = fileId |> IsomFolio.Storage.Db.getFileById c
                match existing with
                | None ->
                    try File.Delete(file); removed <- removed + 1
                    with _ -> ()
                | Some _ -> ()
            return removed
    }
