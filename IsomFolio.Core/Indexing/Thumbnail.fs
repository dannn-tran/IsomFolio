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
    | SetPriority of fileId: FileId * priority: int
    | WorkerDone
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

    // Priority queue: SortedDictionary<priority, Queue<ThumbnailRequest * int>>
    let queue    = SortedDictionary<int, Queue<ThumbnailRequest * int>>()
    let inFlight = System.Collections.Concurrent.ConcurrentDictionary<FileId, bool>()
    let queued   = HashSet<FileId>()
    let mutable activeCount = 0

    let enqueueItem (req: ThumbnailRequest) (retryCount: int) =
        if not (inFlight.ContainsKey(req.FileId)) && not (queued.Contains(req.FileId)) then
            if not (queue.ContainsKey(req.Priority)) then
                queue[req.Priority] <- Queue<ThumbnailRequest * int>()
            queue[req.Priority].Enqueue(req, retryCount)
            queued.Add(req.FileId) |> ignore

    let dequeueItem () =
        let result =
            queue
            |> Seq.tryFind (fun kv -> kv.Value.Count > 0)
            |> Option.map (fun kv -> kv.Value.Dequeue())
        
        result |> Option.iter (fun (work, _) -> queued.Remove(work.FileId) |> ignore)

        // Prune empty buckets
        let emptyKeys = 
            queue 
            |> Seq.filter (fun kv -> kv.Value.Count = 0)
            |> Seq.map (fun kv -> kv.Key)
            |> Seq.toList
        emptyKeys |> List.iter (queue.Remove >> ignore)
        result

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
                | SetPriority(fileId, priority) ->
                    let mutable existingWork = None
                    let mutable foundInKey = None
                    
                    for kv in queue do
                        if foundInKey.IsNone then
                            let found = kv.Value |> Seq.tryFind (fun (r, _) -> r.FileId = fileId)
                            if found.IsSome then
                                foundInKey <- Some kv.Key

                    match foundInKey with
                    | Some key ->
                        let oldQ = queue[key]
                        let items = oldQ |> Seq.toList
                        oldQ.Clear()
                        for (r, rc) in items do
                            if r.FileId = fileId then
                                existingWork <- Some ({ r with Priority = priority }, rc)
                            else
                                oldQ.Enqueue(r, rc)
                        
                        // Prune bucket if now empty
                        if oldQ.Count = 0 then queue.Remove(key) |> ignore
                    | None -> ()
                    
                    match existingWork with
                    | Some (req, rc) -> 
                        queued.Remove(fileId) |> ignore
                        enqueueItem req rc
                    | None -> ()
                    
                    dispatchRemaining ()
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
