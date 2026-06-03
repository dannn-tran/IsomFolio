# IsomFolio — Architecture

Design decisions and subsystem contracts. Describes WHY, not WHAT. Line references are avoided; invariants survive refactors.

---

## Crate structure

```
isomfolio-core          Domain logic, storage, indexing — no UI
isomfolio-app           Iced UI, state machine, messages — no direct DB/scanner calls
isomfolio-extension-host  Launches extension subprocesses; owns the IEP protocol
isomfolio-extension-sdk   C# NuGet package for extension authors
extensions/Faces          Example C# extension: InsightFace ONNX face clustering
```

**Boundary rule:** `isomfolio-app` never calls `db::` or `scanner::` directly. All catalog operations go through `Catalog` (`isomfolio-core/src/catalog.rs`). This keeps the app layer testable and prevents UI code from encoding storage decisions.

---

## State model

IsomFolio uses the Elm architecture via iced:

- **`App`** (`src/app/mod.rs`) — single source of runtime state
- **`Msg`** (`src/app/types.rs`) — exhaustive enum of all events
- **`update`** (`src/app/update/`) — pure-ish reducers; async via `Task`
- **`view`** (`src/view/`) — pure render from state

Keyboard shortcuts are defined declaratively in `src/app/keybinds.rs` and the shortcut help panel auto-generates from the same data. New shortcuts go in the table, not in the subscription closure.

---

## UI rendering & interaction

*Visual and interaction rules — what things look like and how they respond — live in `design-system.md`. This section is the* why *behind the non-obvious UI mechanics, as refactor-surviving invariants.*

### Grid layout & hit-testing

The photo grid is **virtualised**: only the rows intersecting the viewport are built, with spacer blocks above/below for scroll extent. Because tiles aren't real widgets at fixed positions, a cursor position is mapped to a tile by **arithmetic**, not widget hit-testing — so every fixed band stacked above the grid (menu bar, search toolbar, the always-visible cull strip, the optional criteria panel) must have a **known height** that is subtracted from the cursor's Y before the row/column math. **Invariant:** any always-visible element added above the grid has a fixed, constant height folded into that offset; a variable-height band above the grid breaks tile hit-testing. (This is why the cull strip is a fixed single row rather than wrapping content.)

### Folder tree

Built from the distinct indexed folder paths, reconstructing intermediate ancestors into a navigable tree. **Anchored at the library roots**: the forest roots are the *anchors* — the deepest common ancestor of the added folders on each drive (`library_anchors`; a single anchor when they share a prefix, one per top-level segment when they span drives, possibly *virtual* if no added folder sits exactly there). Everything above the anchor (the noisy `/Users/me` prefix) is hidden, so breadcrumbs start at the user's content. Below an anchor, pure pass-through runs (folders with no own photos and exactly one child) are **compacted** into a single breadcrumb row — VS Code "compact folders" — so a deep single chain shows as `a / b / c` on one line, each segment separately clickable (`FolderNode.chain` holds the segments; `path`/`name` are the deepest). The intermediate names stay visible (not discarded). The leading separator of an absolute path is *not* a node — empty path segments are dropped at build time, so there is no nameless "ghost" root (the absolute prefix is re-attached when building top-level paths). With no known roots (unit tests) it falls back to filesystem-top roots. Counts shown are recursive (folder + all descendants). **Invariants:** expansion state is *ephemeral* (UI-only, not persisted); scan depth (recursive vs flat) is decided once when a root is added and *persisted per root*, and re-sync honours it. After a sync the freshly-added folder is revealed by seeding `expand_under_path` with the **normalised** path (it must match the case-folded node paths, dirty set and selection — the raw picker path will silently miss).

#### Path key vs display path

Every stored path exists in two forms. The **key** (`files.folder`, `files.path`, `library_roots.path`, the `compute_file_id` input) is canonicalised *and* case-folded on case-insensitive filesystems (macOS/Windows) — see `path_utils::normalize_path`. This is what makes file identity stable and folder/`LIKE` matching reliable: the OS hands the same file back under different casings across pickers, drag-drop, watcher events and re-scans, and a single folded key collapses them to one row. **It is never user-facing.** The **display path** (`files.folder_display`, `library_roots.path_display`, `path_utils::display_path`) is the same canonicalised path with **original casing preserved**, captured at scan time when the folder is provably online. The folder tree keys its trie on the folded path but takes each node's name from the display path (they share structure, so they align segment-for-segment). **Never derive display names by re-reading disk at render time** — it costs a `canonicalize` per node every sidebar build and returns *lowercase* for offline/missing folders (exactly the ones you most want named). Because the name is stored at scan time, offline folders still show their real case.

#### Removable drives / offline roots

A library root whose path isn't a directory right now (an unplugged drive) is **offline** — a recoverable state, *distinct* from missing/orphaned (file gone but drive present) and from deleted (virtual). Offline is **derived, never persisted**: `App.offline_roots` is recomputed (a cheap `is_dir` per root) on every sidebar load and by a coarse 5 s `RecheckOfflineRoots` poll that stats off-thread (a dead mount can block) and reloads only when the set changes — so unplug/remount self-heals without user action. Files under an offline root keep `is_orphaned = 0`; the grid tile and the sidebar row derive an "Offline" / `⏏` marker from `is_offline_path` at render time. **Offline must never trigger orphaning**: reconcile bails when its root isn't a directory, so a transient unmount can't mass-orphan a folder. (Orphaning — `is_orphaned`, the "Missing"/Locate flow — is currently inert: nothing calls `reconcile_folder`, so no path sets the flag. Removable-drive identity is path-based, so a drive remounting under a different `/Volumes` name re-imports as new — a known limitation, not yet addressed.)

### Grid selection model

Selection is a pure function of (click/key, current selection, a fixed **anchor**, a moving **lead**, and a **base** snapshot). A range selection is `base ∪ [anchor..=lead]`: the anchor stays put, the clicked/arrow'd end moves, and the range is *recomputed each time* (so it grows **and** shrinks), unioned with the base (so disjoint Cmd-picked tiles survive a subsequent range). The base is snapshotted when the anchor is set (plain/Cmd click) and **reset on any view/folder/search switch** — this is what prevents stale file ids from a previous view leaking into a new selection. Keeping it a pure function (not scattered mutations) is what made it testable.

### Loupe image

Zoom/pan state lives in **app state, not inside the image widget**. The custom `LoupeImage` widget is a thin renderer driven by that state and emitting gesture deltas back — done specifically because iced's built-in `image::Viewer` keeps zoom internal and so can't be driven by the on-screen zoom buttons; app-owned state lets buttons, scroll, and drag share one source of truth. Three more invariants:

- **RAW decode is preview-first.** The fit view uses the embedded preview (instant); the full demosaic is decoded only on first zoom-in (pixel-accurate 100% check). Browsing never pays demosaic cost.
- **Neighbour prefetch.** Adjacent photos are decoded ahead so forward/back navigation is instant.
- **No redundant re-decode.** A freshly decoded handle gets a *new* texture id (the renderer keys textures by id), so re-decoding an already-displayed image forces a re-upload and a visible flicker — navigation reuses the prefetched handle instead of re-decoding.

### Context menu

Right-click (Ctrl+Click aliased to it) opens a non-blocking cursor-anchored overlay; no scrim. It is the *fast* path to entity actions, never the *only* path — the off-row discoverability requirement and the menu mirrors are specified in `design-system.md`.

---

## Sync model

### Core invariant

**The catalog is the source of truth once a file is indexed.** External metadata (XMP sidecar, embedded XMP, Apple Finder xattrs) feeds the catalog at first detection only. User actions in IsomFolio are always authoritative and are never overwritten by subsequent filesystem reads.

### Data categories

| Category | Examples | Sync behaviour |
|---|---|---|
| File identity | path, filename, folder, size, mtime, exif_date | Always refreshed — derived facts with no user input |
| Catalog metadata | rating, flag, title, tags, EXIF tech, GPS | Imported once on first detection; never overwritten on re-sync |
| File presence | is_orphaned | Auto-orphaned on deletion; removal from catalog is explicit only |

### Event handling

| Event | Action |
|---|---|
| File created (watcher) | **Structural** — mark the folder dirty (accent dot). Not indexed until the user syncs. |
| File modified (watcher) | **Content-only, same path** — refresh file identity + regenerate the thumbnail. No metadata writes; user edits survive. Applied automatically (cache refresh, not a catalog mutation). |
| File deleted (watcher) | **Structural** — mark the folder dirty. Orphaning happens on sync, not on the raw event. |
| File renamed/moved | **Structural** — mark both folders dirty; resolved on sync. User uses Locate… to recover metadata. |
| XMP sidecar changed | Not watched — explicit via right-click → Import XMP metadata |
| Sync Folder (user) | Applies structural changes: new → first-sync rules; missing → orphan; existing → identity refresh. Clears the folder's dirty state. |

**Watcher is a dirty flag, not a reconciler.** Structural changes (add / delete / rename) are *surfaced* — a dirty dot on the folder — and applied only when the user syncs, so a transient unmount or move never silently orphans records or imports junk. The one exception is a pure content edit of an already-tracked file: that has no structural effect and no metadata risk, so its thumbnail is refreshed in place. There is no auto-reconcile on startup.

### XMP precedence

Sidecar `.xmp` fully wins over embedded XMP when present — they are not merged. Matches Lightroom Classic, Bridge, and digiKam convention.

### First-sync import settings

Three-state `Option<bool>` in `AppSettings`:
- `None` — undecided; next sync will prompt
- `Some(true)` — auto-import on first detection
- `Some(false)` — never auto-import (explicit right-click still works)

Cancelling the prompt leaves settings at `None` so the prompt reappears. Settings are global (not per-catalog) so the user doesn't answer the same question for every new catalog.

### Trade-offs

| Pro | Con |
|---|---|
| User edits can't be silently overwritten by a sync | Users editing XMP externally must explicitly re-import |
| Eliminates "where did my rating go?" bugs | Right-click import actions to discover |
| Nothing implicit happens on sync after first import | First-sync prompt adds an onboarding step |

---

## Tag model

Tags have an `origin` column distinguishing their provenance:

| Origin | Write path | Semantics |
|---|---|---|
| `manual` | `upsert_tags` | User-assigned; authoritative; never auto-removed |
| `ai` | `add_tags_merge` / `insert_pending_tags` | AI-suggested; shown with confidence; user confirms or rejects |
| `xmp` | `sync_xmp_tags` | Imported from XMP `dc:subject`; additive, never removes existing tags |
| `apple` | `sync_apple_tags` | Imported from macOS Finder tags; additive |

AI-origin tags enter as *pending* and surface in the Suggestions sidebar item. Accepting promotes them to `manual`. Rejecting discards them. The origin is preserved after promotion — do not overwrite origin on subsequent reads.

---

## Virtual delete

Deleting a photo sets an `is_deleted` flag on the `files` row — **the file on disk is never moved or removed.** Deleted rows are excluded from every normal query/count (`is_deleted = 0`, threaded like `is_orphaned`); the Deleted view inverts it (`only_deleted`). Restore just clears the flag, so it's instant and lossless (the row, with its rating/tags, never left the catalog).

**Critical invariant — the flag survives re-sync.** `upsert_files` therefore does *not* wholesale-replace rows: it `INSERT … ON CONFLICT(id) DO UPDATE` refreshing only the identity columns read from disk, while **preserving** `flag`, `is_deleted`, and `created_at_unix` (catalog add-time). A freshly scanned `AssetFile` carries none of that user/catalog state, so a wholesale replace would silently wipe flags and resurrect deleted photos on every sync. (Fixing this also fixed a pre-existing bug where re-sync wiped culling flags.) Permanent purge is the only operation that may touch the file on disk, and only as an explicit, separate action.

---

## Extension system

### Design intent

AI capabilities are opt-in runtime plugins, not compiled into the base binary. Extensions are separate executables (any language) distributed as `.isfx` zip packages containing a manifest, native binaries, and ONNX models.

### Lifecycle

1. **Install**: `isomfolio-extension-host` extracts the `.isfx`, runs the executable with `--setup --data-dir <data_dir>` to download models and validate the environment. The host reads the exit code; all diagnostic output comes via stderr.
2. **Runtime**: The host launches the executable as a child process with `--data-dir <data_dir>`. Communication is over stdin/stdout (IEP protocol) and stderr (diagnostics).
3. **Uninstall**: Extension directory deleted; `data-dir` (models, cache) is separate and preserved across reinstalls.

### IsomFolio Extension Protocol (IEP)

**Channel split — invariant:**
- **stdout** = protocol only. Newline-delimited JSON objects. Never write diagnostics here.
- **stderr** = structured JSON diagnostics. Parsed by the host and formatted as `[extension] [level] component: message`. Never write protocol messages here.

Mixing the channels breaks parsing on both sides and is a hard protocol violation.

**Handshake (startup):**
```
host → subprocess:  {"type":"ping","id":1}
subprocess → host:  {"type":"ready","extension":"faces","version":"1.0.0"}
```

**Request/response (runtime):**
```
host → subprocess:  {"type":"request","id":2,"method":"cluster_faces","params":{...}}
subprocess → host:  {"type":"response","id":2,"result":{...}}
                 or {"type":"fatal","message":"..."}  (triggers restart/error display)
```

**Batch vs single calls — invariant:**

- `send_many()` for batch work (pipelined individual requests, e.g., classify many images). The host sends all requests without waiting for responses, then collects.
- `call()` / `call_long()` for single operations (e.g., face clustering over the whole library). `call_long()` raises the timeout.
- Never use `call()` in a loop for batch work — it serialises requests and is orders of magnitude slower.

### Extension data directories

Two separate directories are passed to the extension:

| Directory | Purpose | Lifetime |
|---|---|---|
| `ext_dir` (extension root) | `config.json`, manifest, binary | Deleted on uninstall |
| `data_dir` | ONNX models, `state.db` embedding cache | Persists across reinstalls |

This split allows re-installing an extension (e.g., after a version update) without losing the model download or embedding cache.

---

## Face clustering specifics

The host owns clustering; the inference engine only embeds faces (`POST /embed`). Two things are incremental:

- **Embedding** — only cache-miss files are sent to the engine (`get_uncached_face_file_paths`); embeddings persist in `face_embeddings`. The expensive inference never repeats for an unchanged file.
- **Clustering** — `clustering::cluster_incremental` assigns each face to the nearest *existing* centroid within `eps` (keeping that cluster's id so named people stay named), then runs DBSCAN over only the *unassigned* faces to discover brand-new people. Cost is ~O(n·k) assignment + O(m²) over the m leftovers, versus O(n²) for a full DBSCAN.

The manual ⟳ button runs a **full** re-cluster (`force_full: true`, DBSCAN over everything) — needed after changing `face_eps` / `face_min_pts`. The **incremental** path runs automatically after a sync that finds new files (when `auto_face_cluster` is on, the default), so adding photos surfaces new people cheaply without a full rebuild. Cluster names survive id changes either way: `save_face_clusters` re-associates names to the new cluster with maximum membership overlap.

The ORT (ONNX Runtime) sessions for detection and recognition must have `EnableCpuMemArena = false` and `EnableMemoryPattern = false`. The default arena grows native heap across `Run()` calls and never releases to the OS, causing exhaustion over a large batch. This was validated via a 50-image stress test.

---

## Preferred extension per capability

When multiple extensions claim the same capability (e.g., `classify`), the app uses a `HashMap<capability, extension_name>` preference stored in global app settings (not the catalog DB — preferences should be global, not per-catalog). Surfaced in Settings → Extensions as a chip selector when two or more extensions share a capability.

Auto-tag on scan always uses the preferred extension for `classify`. Context menu shows all matching extensions for manual runs.
