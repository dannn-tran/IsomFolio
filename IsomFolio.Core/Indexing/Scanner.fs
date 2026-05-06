module IsomFolio.Core.Indexing.Scanner

open System
open System.Collections.Generic
open System.IO
open System.Threading.Channels
open System.Threading.Tasks
open FSharp.Control
open IsomFolio.Core.Indexing.Types
open IsomFolio.Core.Metadata
open IsomFolio.Core.Models
open IsomFolio.Core.FileIndex
open IsomFolio.Core.PathUtils
open IsomFolio.Core.Storage
open Microsoft.Data.Sqlite

type ScannedFile = {
    Asset    : AssetFile
    Metadata : EmbeddedMetadata
}

/// Describes the work to perform per file path. Returns None for unsupported or unreadable files.
type ScanJob = string -> Async<ScannedFile option>

type BulkFileLoader = string seq -> IAsyncEnumerable<ScannedFile>

/// Default job: build AssetFile from FileInfo then read all metadata sources in parallel.
let defaultJob : ScanJob = fun path ->
    async {
        let fileAsset =
            try
                let fi = FileInfo(path)
                if isSupportedExtension fi.Extension then Some (assetFileFromInfo fi)
                else None
            with ex ->
                eprintfn "Scanner: skipping %s — %s" path ex.Message
                None
        match fileAsset with
        | None -> return None
        | Some asset ->
            let! meta = EmbeddedMetadata.read path
            return Some { Asset = asset; Metadata = meta }
    }

let private discoverPaths (rootPath: string) : string seq =
    try Directory.EnumerateFiles(rootPath, "*.*", SearchOption.AllDirectories)
    with ex ->
        eprintfn "Scanner: cannot enumerate %s — %s" rootPath ex.Message
        Seq.empty

let private chunked (n: int) (source: IAsyncEnumerable<'a>) : IAsyncEnumerable<'a list> =
    taskSeq {
        let buf = List<'a>()
        for item in source do
            buf.Add(item)
            if buf.Count >= n then
                yield buf |> Seq.toList
                buf.Clear()
        if buf.Count > 0 then
            yield buf |> Seq.toList
    }

/// Sequential execution: one file at a time; per-file metadata sub-reads run concurrently.
let runSequential (job: ScanJob) : BulkFileLoader = fun paths ->
    taskSeq {
        for path in paths do
            let! result = job path |> Async.StartAsTask
            match result with
            | Some f -> yield f
            | None   -> ()
    }

/// Parallel execution: up to `parallelism` files processed concurrently.
/// Workers are suspended (not blocked) during I/O, so no thread pool threads are wasted.
let runParallel (parallelism: int) (job: ScanJob) : BulkFileLoader = fun paths ->
    taskSeq {
        let channel =
            Channel.CreateBounded<ScannedFile>(
                BoundedChannelOptions(
                    parallelism * 2,
                    FullMode = BoundedChannelFullMode.Wait))

        let producer =
            task {
                let opts = ParallelOptions(MaxDegreeOfParallelism = parallelism)
                try
                    do! Parallel.ForEachAsync(paths, opts, fun path _ ->
                        task {
                            let! result = job path |> Async.StartAsTask
                            match result with
                            | Some f -> do! channel.Writer.WriteAsync(f)
                            | None   -> ()
                        } |> ValueTask)
                finally
                    channel.Writer.Complete()
            }

        while! channel.Reader.WaitToReadAsync() do
            let mutable item = Unchecked.defaultof<_>
            while channel.Reader.TryRead(&item) do
                yield item

        do! producer
    }

let enumerateFiles (loadFiles: BulkFileLoader) (batchSize: int) (rootPath: string) : IAsyncEnumerable<ScannedFile list> =
    rootPath
    |> discoverPaths
    |> loadFiles
    |> chunked batchSize

/// Convenience wrapper matching the original signature.
/// Metadata is read but not yet persisted; schema support pending.
let scanFolder
    (rootPath   : string)
    (onBatch    : AssetFile list -> Async<unit>)
    (onProgress : ScanProgress -> unit)
    : Async<ScanResult> =
    async {
        let mutable total = 0

        for batch in enumerateFiles (runSequential defaultJob) 500 rootPath do
            do! onBatch (batch |> List.map _.Asset)
            total <- total + batch.Length
            onProgress {
                TotalFound = total
                Inserted   = total
                FolderName = Path.GetFileName(rootPath)
            }

        return { TotalCount = total }
    }

let reconcileFolder (c: SqliteConnection) (rootPath: string) : Async<string list * string list> =
    async {
        let! indexed = Db.getIndexedPathsInFolder c rootPath

        let fsFiles = System.Collections.Generic.Dictionary<string, FileInfo>()
        try
            for filePath in Directory.EnumerateFiles(rootPath, "*.*", SearchOption.AllDirectories) do
                try
                    let fi = FileInfo(filePath)
                    if isSupportedExtension fi.Extension then
                        fsFiles[normalizePath fi.FullName] <- fi
                with _ -> ()
        with ex ->
            eprintfn "Reconcile: cannot enumerate %s — %s" rootPath ex.Message

        let newOrModified =
            fsFiles
            |> Seq.choose (fun kv ->
                let fi = kv.Value
                match indexed |> Map.tryFind kv.Key with
                | None -> Some fi.FullName
                | Some existing ->
                    let mtime = DateTimeOffset(fi.LastWriteTimeUtc).ToUnixTimeSeconds()
                    if existing.MTimeUnix <> mtime || existing.SizeBytes <> fi.Length
                    then Some fi.FullName
                    else None)
            |> Seq.toList

        let orphaned =
            indexed
            |> Map.toSeq
            |> Seq.choose (fun (path, file) ->
                if not (fsFiles.ContainsKey(path)) && not file.IsOrphaned
                then Some file.Id
                else None)
            |> Seq.toList

        return newOrModified, orphaned
    }
