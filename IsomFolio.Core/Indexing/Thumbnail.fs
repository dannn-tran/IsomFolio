module IsomFolio.Core.Indexing.Thumbnail

open System
open System.IO
open System.Collections.Generic
open IsomFolio.Core.Indexing.Types
open Microsoft.Data.Sqlite
open SkiaSharp
open IsomFolio.Core.Models
open IsomFolio.Core.AppPaths

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

module private Async =
    let map f a = async.Bind(a, f >> async.Return)

    let withTimeout (timeoutMs: int, computation: Async<'T>) : Async<'T> =
        async {
            let! child = Async.StartChild(computation, timeoutMs)
            try
                return! child
            with :? TimeoutException ->
                return raise (TimeoutException $"Thumbnail generation timed out after {timeoutMs}ms")
        }

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

                // Wrap the decode/resize/save in an async with a timeout to prevent hanging on corrupt files
                let work = async {
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
                }
                
                let withTimeout = 
                    async {
                        try
                            let! res = Async.withTimeout (30000, work)
                            return Choice1Of2 res
                        with ex ->
                            return Choice2Of2 ex
                    }

                return! withTimeout |> Async.map (function 
                            | Choice1Of2 (Ok path) -> Ok path
                            | Choice1Of2 (Error e) -> Error e
                            | Choice2Of2 ex -> Error ex.Message)
            with ex ->
                return Error ex.Message
    }

// ---------------------------------------------------------------------------
// Worker pool
// ---------------------------------------------------------------------------

type ThumbnailMsg =
    | Enqueue     of req: ThumbnailRequest * retryCount: int
    | WorkerDone
    | CancelAll
    | Shutdown

/// Creates a background worker pool backed by a MailboxProcessor queue.
/// concurrency: number of parallel workers (default 4).
/// onReady / onFailed: callbacks invoked on the calling context (route through Dispatcher.UIThread.Post in App layer).
let createWorkerPool
    (catalogDir   : string)
    (concurrency  : int)
    (onReady  : FileId -> string -> unit)
    (onFailed : FileId -> string -> unit)
    : MailboxProcessor<ThumbnailMsg> =

    // Simple queue
    let queue    = Queue<ThumbnailRequest * int>()
    let inFlight = System.Collections.Concurrent.ConcurrentDictionary<FileId, bool>()
    let queued   = HashSet<FileId>()
    let mutable activeCount = 0

    let enqueueItem (req: ThumbnailRequest) (retryCount: int) =
        if not (inFlight.ContainsKey(req.FileId)) && not (queued.Contains(req.FileId)) then
            queue.Enqueue(req, retryCount)
            queued.Add(req.FileId) |> ignore

    let dequeueItem () =
        if queue.Count > 0 then
            let work, rc = queue.Dequeue()
            queued.Remove(work.FileId) |> ignore
            Some (work, rc)
        else
            None

    let agent = MailboxProcessor.Start(fun inbox ->
        let rec startWorker (work, retryCount) =
            activeCount <- activeCount + 1
            inFlight[work.FileId] <- true
            Async.Start(async {
                try
                    try
                        let! result = generateThumbnail catalogDir work
                        match result with
                        | Ok path  -> 
                            onReady work.FileId path
                        | Error msg ->
                            if retryCount < 1 then
                                // Release worker immediately, schedule retry later
                                inFlight.TryRemove(work.FileId) |> ignore
                                Async.Start(async {
                                    do! Async.Sleep 5000
                                    inbox.Post (Enqueue(work, retryCount + 1))
                                })
                            else
                                onFailed work.FileId msg
                    with ex ->
                        onFailed work.FileId ex.Message
                finally
                    inFlight.TryRemove(work.FileId) |> ignore
                    inbox.Post WorkerDone
            })
        let rec dispatchRemaining () =
            if activeCount < concurrency then
                match dequeueItem () with
                | Some item ->
                    startWorker item
                    dispatchRemaining ()
                | None -> ()

        let rec loop () =
            async {
                let! msg = inbox.Receive()
                match msg with
                | Shutdown -> ()
                | WorkerDone ->
                    activeCount <- max 0 (activeCount - 1)
                    dispatchRemaining ()
                    return! loop ()
                | CancelAll ->
                    queue.Clear()
                    queued.Clear()
                    return! loop ()
                | Enqueue (req, rc) ->
                    enqueueItem req rc
                    dispatchRemaining ()
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
            let! allIds = IsomFolio.Core.Storage.Db.getAllFileIds c
            let known = HashSet<string>(allIds)
            let mutable removed = 0
            for file in Directory.EnumerateFiles(cacheDir, "*.jpg") do
                let fileId = Path.GetFileNameWithoutExtension(file)
                if not (known.Contains(fileId)) then
                    try File.Delete(file); removed <- removed + 1
                    with _ -> ()
            return removed
    }
