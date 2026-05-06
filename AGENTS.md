# IsomFolio — Agent Onboarding

> **Keep this file up to date.** When you change architecture, add a module, alter a pipeline, rename a message type, or fix a gotcha — update the relevant section here before closing the task. Stale docs are worse than no docs.

Desktop asset manager in F#. Files/tags live on disk; the catalog is an index, not a store. UI is Elmish MVU on Avalonia FuncUI.

## Project layout

```
IsomFolio.Core/   — pure business logic, no Avalonia dependency
IsomFolio.App/    — Avalonia FuncUI + Elmish 4.x MVU; all UI code lives here
IsomFolio.Tests/  — xUnit; references Core only (no Avalonia)
```

## Catalog format

A catalog is a directory `<name>.isomfolio/` containing:
- `catalog.db` — SQLite (WAL mode)
- `thumbnails/` — JPEG cache, one file per asset named `<fileId>.jpg`

`AppPaths.fs` is the single source of truth for these paths.

## Data model (`Models.fs`)

- **`AssetFile`** — immutable snapshot of a file; one row in the `files` DB table
- **`FileId`** — `string` alias; SHA-256 hex of the file's absolute path
- **`ThumbnailState`** — per-tile state machine: `NotRequested → Pending → Ready(path) | Failed(retryCount)`
- **`SearchQuery`** — all active filter dimensions; `State.ActiveQuery` is the single source of truth

## Architecture overview

**Scan pipeline**: `FolderOpened` → `startScanCmd` (`Cmd.ofEffect` + `Async.Start`) → `Scanner.scanFolder` on thread pool → per 500 files: `Db.upsertFiles` → `ScanBatchCompleted` (via `Dispatcher.UIThread.Post`) → `ScanFinished` triggers `runSearch`.

**Thumbnail pipeline**: `primeGridThumbnails` checks cache; hits dispatch `ThumbnailUpdated(fileId, Ready path)` synchronously; misses set `Pending` and enqueue to a `MailboxProcessor<ThumbnailMsg>` worker pool (default 4 concurrent workers). Workers post results via `Dispatcher.UIThread.Post`.

**Search**: `QueryEngine.executeSearch` builds parameterised SQL dynamically. FTS5 for text search, folder filter includes descendants, tags are AND-semantics inner joins. `SearchRequestId` guards against stale results — always discard if `id ≠ state.SearchRequestId`.

**File watcher**: `Watcher.createWatcher` debounces events (250 ms per path), suppresses self-writes from XMP sidecar updates. Events are handled with different urgency:

| Event | Handling | Rationale |
|---|---|---|
| `Deleted` | Immediate — file marked orphaned in DB, `?` badge shown on tile | User should see missing files right away |
| `Renamed` | Immediate — `Db.updateFilePath` preserves the existing record | Deferred reconcile would treat rename as orphan + new file, losing tags and thumbnail |
| `Created` | Deferred — parent folder added to `State.PendingFolders` | User controls when new files enter the index via the `↻` button in the sidebar |
| `Modified` | Deferred — parent folder added to `State.PendingFolders` | Metadata update, low urgency |

`State.PendingFolders` is a `Set<string>` of normalized folder paths. The sidebar shows an amber `↻` button on any folder node whose subtree contains a pending folder. Clicking it dispatches `ResyncFolderRequested`, which runs `reconcileFolder` for that path and then removes all descendant entries from `PendingFolders`.

**Path normalization gotcha**: `normalizePath` calls `ToLowerInvariant()` on macOS/Windows for stable `FileId` hashing. All paths stored in the DB are lowercase. Watcher events provide original-case paths — always call `normalizePath` before using a watcher path as a DB key (e.g. `Map.tryFind`, `WHERE path = @p`). Never pass a normalized (lowercase) path to `FileInfo()` — `FileInfo.Name` is derived from the constructor argument, so a lowercase path produces a lowercase display name.

**Tagging**: tags written to XMP sidecars, then mirrored to DB + FTS index. Tags travel with files if the catalog is deleted.

---

## MVU rules

The Elmish loop is single-threaded. Keep these roles strict:

| Layer | Rule |
|---|---|
| **Model** | Immutable snapshot. No mutable objects, no disposables, no DB connections. |
| **Update** | Pure function. Zero I/O. Returns `(State * Cmd<Msg>)`. |
| **Commands** | All side effects: async work, DB calls, file I/O, subscriptions. |

**No relay `Msg` cases.** A relay case exists only to re-dispatch as another message with no transformation — pure indirection. Dispatch the final message directly from the worker/callback instead.

OK — structural wrappers (`SidebarMsg`, `GridMsg`, `DetailMsg`, `SearchBarMsg`) that delegate to sub-module `update`.

Not OK — cases like the old `ThumbnailReady` that only re-dispatched as `GridMsg (GridView.ThumbnailUpdated(...))`.

---

## Async conventions

Match the command API to what the work layer returns:

```fsharp
// Work returns Async<'T>
Cmd.OfAsync.either loadAsync () DataLoaded LoadFailed

// Work returns Task<'T>
Cmd.OfTask.either loadAsync () DataLoaded LoadFailed

// Need dispatch inside a side-effecting operation (watcher, worker start)
Cmd.ofEffect (fun dispatch -> ...)
```

**Never** wrap `Async<'T>` with `|> Async.StartAsTask` just to use `Cmd.OfTask` — boilerplate with no benefit.

**Always** use `either`, not `perform`. `perform` silently swallows exceptions.

**Never** `Async.RunSynchronously` on the UI thread — freezes the interface.

---

## Threading rules

**UI thread for dialogs** — `StorageProvider` picker APIs crash on macOS if called off the UI thread (`NSWindow should only be instantiated on the main thread!`). Use `Cmd.ofEffect`:

```fsharp
Cmd.ofEffect (fun dispatch ->
    w.StorageProvider.OpenFolderPickerAsync(opts)
        .ContinueWith(fun (t: Task<_>) ->
            if not t.IsFaulted && not t.IsCanceled && t.Result.Count > 0 then
                Async.Start(async {
                    let! conn = Db.openDatabase path
                    dispatch (CatalogOpened (path, conn, []))
                }))
    |> ignore)
```

Do NOT use `Cmd.OfAsync.either` for dialog calls — it may not guarantee UI-thread execution.

**Background → UI dispatch** — any callback from a background thread must post back before calling `dispatch`:

```fsharp
Dispatcher.UIThread.Post(fun () -> dispatch SomeMsg)
```

Use `Dispatcher.UIThread.Post` only at integration boundaries (scanner, watcher, thumbnail worker callbacks). Prefer Elmish commands for everything else — they dispatch on the correct thread automatically.

---

## SubPatchOptions (critical FuncUI gotcha)

FuncUI's virtual DOM diffs views positionally. Event handlers default to `SubPatchOptions.Never` — the closure is attached once and **never re-bound**, even when the item at that slot changes. This silently produces wrong-element bugs in dynamic lists.

```fsharp
// Wrong — closure captures model.File.Id once at first render
Border.onTapped (fun _ -> dispatch (TileSelected model.File.Id))

// Correct — re-bind whenever the tile's identity changes
Border.onTapped(
    (fun _ -> dispatch (TileSelected model.File.Id)),
    SubPatchOptions.OnChangeOf model.File.Id)
```

Use `SubPatchOptions.OnChangeOf <key>` on any event handler whose closure captures values that can change between renders.

---

## Concurrency guard — request ID pattern

```fsharp
| Load ->
    let id = model.SearchRequestId + 1
    { model with SearchRequestId = id },
    Cmd.OfAsync.either (searchAsync id) () SearchCompleted Failed

| SearchCompleted (id, results) ->
    if id = model.SearchRequestId then
        { model with Grid = ... }, Cmd.none
    else
        model, Cmd.none  // stale — discard
```

**Don't store `CancellationTokenSource` in the model** — it's mutable and `IDisposable`. Manage its lifetime in a `MailboxProcessor` agent.

---

## SQLite conventions

Short-lived connections. Open immediately before the query; dispose immediately after. Never store a `SqliteConnection` in the model.

```fsharp
let loadAsync () = async {
    use! conn = Db.openDatabase path
    return! conn |> Db.getFilesByFolder folder
}
```

Grouped writes go inside a single transaction. Enable WAL on first open (already done in `Db.openDatabase`). Use `busy_timeout` for concurrent readers.

---

## F# language gotchas in this codebase

**Inline async blocks** fail with FS0020/FS0748 — always expand to multiline:

```fsharp
// fails
async { do! Async.Sleep 5000; return t }

// correct
async {
    do! Async.Sleep 5000
    return t
}
```

**Or-patterns with complex nested patterns** — F# cannot group `SidebarMsg Sidebar.X` inside a parenthesised or-group:

```fsharp
// wrong — parser error
| { Window = w }, (NewCatalogRequested | OpenCatalogRequested) -> ...

// right
| { Window = w }, NewCatalogRequested
| { Window = w }, OpenCatalogRequested -> ...
```

**macOS NSSavePanel compact mode** — first use shows save panel in compact mode. No Avalonia API forces expanded mode. User clicks the disclosure triangle; state persists after first expansion.

---

## Testing approach

Test `Core` logic and `update` functions — both are pure and require no UI infrastructure.

**Run tests**: `dotnet test`

**Structure**: group tests for the same function/module into a nested module when there are 3+ tests.

**Per-test isolation**: each test gets its own fresh temp DB (unique GUID path). Never share state between tests.

**Prefer integration over mocks**: DB tests use real SQLite on temp files; scanner tests use real temp directories.

**Test `update` directly** — no Elmish harness needed:

```fsharp
let state = { GridView.init () with SelectedId = Some file.Id }
let nextState = GridView.update (GridView.TilesLoaded [file]) state
Assert.Equal(Some file.Id, nextState.SelectedId)
```

**Test commands** by executing them and collecting dispatched messages:

```fsharp
let execCmd (cmd: Cmd<Msg>) = async {
    let messages = ConcurrentQueue<Msg>()
    let tcs = TaskCompletionSource()
    let dispatch msg = messages.Enqueue(msg); tcs.TrySetResult() |> ignore
    for sub in cmd do sub dispatch
    do! tcs.Task.WaitAsync(TimeSpan.FromSeconds(2.0)) |> Async.AwaitTask
    return messages.ToArray() |> Array.toList
}
```

**TDD workflow**: write the test first against the intended `update` logic or `Db.*` function, confirm it fails, then implement.

---

## MailboxProcessor agent patterns

These patterns apply whenever building a background worker (thumbnail pool, metadata extractor, file watcher coordinator, etc.).

**Non-blocking agent loop (WorkerDone pattern)** — never `let!`/`do!` a long-running task or semaphore inside the `inbox.Receive()` loop. The agent stops processing all messages while awaiting:

```fsharp
// Wrong — blocks the entire agent while waiting for a slot
let! _ = semaphore.WaitAsync() |> Async.AwaitTask
doWork ()

// Correct — agent stays responsive; tracks count manually
let rec loop activeCount queue = async {
    let! msg = inbox.Receive()
    match msg with
    | Enqueue work when activeCount < limit ->
        Async.Start(async {
            try doWork work
            finally inbox.Post WorkerDone
        })
        return! loop (activeCount + 1) queue
    | WorkerDone ->
        return! loop (activeCount - 1) queue
    ...
}
```

**Batch state updates** — collect all changes, then apply in one pass. Calling `GridView.update` per tile inside a loop creates O(N²) state copies:

```fsharp
// Wrong — 1000 update calls = 1000 state copies
for file in batch do
    state <- GridView.update (ThumbnailUpdated file) state

// Correct — build a lookup, one pass over tiles
let lookup = batch |> Map.ofList
let tiles = state.Tiles |> List.map (fun t ->
    match Map.tryFind t.File.Id lookup with
    | Some thumb -> { t with Thumbnail = thumb }
    | None -> t)
{ state with Tiles = tiles }
```

**Decoupled retries** — release the worker slot before sleeping; post back to the agent queue for retry. Sleeping inside a worker starves other files of concurrency:

```fsharp
// Wrong — slot occupied during 5s sleep
do! Async.Sleep 5000
retry work

// Correct — release immediately, re-enqueue after delay
Async.Start(async {
    do! Async.Sleep 5000
    inbox.Post(Enqueue(work, retryCount + 1))
})
```

**Defensive try/finally in workers** — always send `WorkerDone` even on exception, or slots leak permanently:

```fsharp
Async.Start(async {
    try
        do! doWork work
    finally
        inbox.Post WorkerDone
})
```

---

## Product scope

**Supported image formats**: `jpg`, `jpeg`, `png`, `webp`, `gif` — defined in `FileIndex.supportedExtensions`. Video is not supported.

**Performance targets**:
- Folder load (previously indexed): < 300ms
- Search (≤ 100k files, ≤ 10 tags/file): < 100ms
- UI thread block from I/O or decode: 0ms — never

**Known gaps / not yet implemented**:
- Thumbnail cache invalidation on file modification (currently cache-presence only)
- Sidecar-to-SQLite tag import during scan (tags only populate when edited in-app)
- `FileSystemWatcher` overflow recovery / polling fallback
- Watcher failure UI and manual re-scan
- FTS5 index rebuild UI

**MVP non-goals** (do not partially implement):
- Cloud sync, AI tagging, face/object recognition, video playback
- Multi-user collaboration, plugin/scripting system
- Hierarchical tags, duplicate detection, batch tagging UI
- Cross-platform library portability, database encryption

---

## Anti-patterns

| Anti-pattern | Problem | Fix |
|---|---|---|
| `SqliteConnection` in model | Mutable, disposable — breaks immutability | Short-lived connections per operation |
| `CancellationTokenSource` in model | Mutable, leaks on every model update | Manage lifetime in a `MailboxProcessor` agent |
| I/O inside `update` | Breaks purity, blocks UI | Move to `Cmd.OfAsync.either` |
| `Cmd.OfAsync.perform` / `.perform` for fallible work | Swallows exceptions silently | Use `either` |
| `Async.RunSynchronously` on UI thread | Freezes the interface | Use async commands |
| `Cmd.OfAsync.either` for dialogs | Not guaranteed UI-thread | Use `Cmd.ofEffect` |
| `Dispatcher.UIThread.Post` as convenience | Bypasses Elmish loop | Reserve for integration boundaries only |
| Event handler without `SubPatchOptions` in dynamic lists | Stale closure, wrong element fires | `SubPatchOptions.OnChangeOf <id>` |
| Relay `Msg` case | Pure indirection, no transform | Dispatch final message directly |
| Subscription returning no-op disposable | Resource/timer leak | Always stop and dispose the underlying resource |
| Parallel writes to SQLite without serialisation | `SQLITE_BUSY` errors | Use `busy_timeout`; serialise via agent for high contention |
