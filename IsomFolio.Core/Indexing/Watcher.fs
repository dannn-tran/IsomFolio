module IsomFolio.Core.Indexing.Watcher

open System
open System.IO
open System.Collections.Generic
open IsomFolio.Core.Indexing.Types

// Track when IsomFolio last wrote an XMP sidecar so we can suppress self-triggered events
let private selfWrites = Dictionary<string, DateTime>()
let private selfWriteLock = obj ()

/// Call before writing an XMP sidecar to register a self-write (suppresses the resulting watcher event)
let registerSelfWrite (xmpPath: string) =
    lock selfWriteLock (fun () ->
        selfWrites[xmpPath] <- DateTime.UtcNow)

let private isSelfWrite (path: string) =
    lock selfWriteLock (fun () ->
        match selfWrites.TryGetValue(path) with
        | true, ts when (DateTime.UtcNow - ts).TotalMilliseconds < 500.0 ->
            selfWrites.Remove(path) |> ignore
            true
        | _ -> false)

/// Resolve a .xmp path to the corresponding image path by checking each supported extension.
/// Returns None if no supported image exists at that base path.
let private resolveXmpToImage (xmpPath: string) : string option =
    let basePath = Path.ChangeExtension(xmpPath, null)
    [ "jpg"; "jpeg"; "png"; "webp"; "gif" ]
    |> List.tryPick (fun ext ->
        let candidate = basePath + "." + ext
        if File.Exists(candidate) then Some candidate else None)

/// Create a FileSystemWatcher for rootPath that fires debounced FileEvents via dispatch.
/// The returned watcher is already started; call stopWatcher to dispose it.
let createWatcher (rootPath: string) (dispatch: FileEvent -> unit) : FileSystemWatcher =
    let debounceMs = 250
    let timers = Dictionary<string, System.Threading.Timer>()
    let timersLock = obj ()

    let fire (event: FileEvent) () =
        if not (isSelfWrite (
            match event with
            | Created p | Deleted p | Modified p -> p
            | Renamed(_, np) -> np
            | SidecarChanged p | SidecarRemoved p -> p)) then
            dispatch event

    let debounce (path: string) (event: FileEvent) =
        lock timersLock (fun () ->
            match timers.TryGetValue(path) with
            | true, existing -> existing.Dispose()
            | _ -> ()
            let t = new System.Threading.Timer(
                        (fun _ -> lock timersLock (fun () -> timers.Remove(path) |> ignore)
                                  fire event ()),
                        null,
                        debounceMs,
                        System.Threading.Timeout.Infinite)
            timers[path] <- t)

    let watcher = new FileSystemWatcher(rootPath)
    watcher.IncludeSubdirectories <- false
    watcher.InternalBufferSize    <- 65536
    watcher.NotifyFilter          <-
        NotifyFilters.LastWrite ||| NotifyFilters.FileName ||| NotifyFilters.DirectoryName

    watcher.Created.Add(fun e ->
        if e.FullPath.EndsWith(".xmp", StringComparison.OrdinalIgnoreCase) then
            resolveXmpToImage e.FullPath
            |> Option.iter (fun imgPath -> fire (SidecarChanged imgPath) ())
        else
            debounce e.FullPath (Created e.FullPath))

    watcher.Deleted.Add(fun e ->
        if e.FullPath.EndsWith(".xmp", StringComparison.OrdinalIgnoreCase) then
            resolveXmpToImage e.FullPath
            |> Option.iter (fun imgPath -> fire (SidecarRemoved imgPath) ())
        else
            debounce e.FullPath (Deleted e.FullPath))

    watcher.Changed.Add(fun e ->
        if e.FullPath.EndsWith(".xmp", StringComparison.OrdinalIgnoreCase) then
            resolveXmpToImage e.FullPath
            |> Option.iter (fun imgPath -> fire (SidecarChanged imgPath) ())
        else
            debounce e.FullPath (Modified e.FullPath))

    watcher.Renamed.Add(fun e -> debounce e.FullPath (Renamed(e.OldFullPath, e.FullPath)))

    watcher.Error.Add(fun e ->
        eprintfn "Watcher error on %s: %s" rootPath (e.GetException().Message))

    watcher.EnableRaisingEvents <- true
    watcher

let stopWatcher (watcher: FileSystemWatcher) =
    watcher.EnableRaisingEvents <- false
    watcher.Dispose()
