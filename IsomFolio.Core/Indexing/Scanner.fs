module IsomFolio.Core.Indexing.Scanner

open System
open System.IO
open IsomFolio.Core.Indexing.Types
open IsomFolio.Core.Models
open IsomFolio.Core.FileIndex
open IsomFolio.Core.PathUtils
open IsomFolio.Core.Storage
open Microsoft.Data.Sqlite

/// Recursively scan a folder, batch-insert into the DB, and report progress.
/// onBatch is called with each batch of 500 files — caller routes to DB upsert.
/// onProgress is called after each batch with running totals.
let scanFolder
    (rootPath   : string)
    (onBatch    : AssetFile list -> Async<unit>)
    (onProgress : ScanProgress -> unit)
    : Async<ScanResult> =
    async {
        let buffer = System.Collections.Generic.List<AssetFile>()
        let mutable totalInserted = 0

        let flush () =
            async {
                if buffer.Count > 0 then
                    let batch = buffer |> Seq.toList
                    buffer.Clear()
                    do! onBatch batch
                    totalInserted <- totalInserted + batch.Length
                    onProgress { TotalFound = totalInserted; Inserted = totalInserted; FolderName = Path.GetFileName(rootPath) }
            }

        let files =
            try
                Directory.EnumerateFiles(rootPath, "*.*", SearchOption.AllDirectories)
            with ex ->
                eprintfn "Scanner: cannot enumerate %s — %s" rootPath ex.Message
                Seq.empty

        for filePath in files do
            try
                let fi = FileInfo(filePath)
                if isSupportedExtension fi.Extension then
                    buffer.Add(assetFileFromInfo fi)
                    if buffer.Count >= 500 then
                        do! flush ()
            with ex ->
                eprintfn "Scanner: skipping %s — %s" filePath ex.Message

        do! flush ()

        return { TotalCount = totalInserted }
    }

/// Compare filesystem state against DB records for a root folder.
/// Returns lists of paths that are new-or-modified and FileIds that are orphaned.
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

        // Files on disk not in DB, or with changed mtime/size
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

        // Files in DB not found on disk
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
