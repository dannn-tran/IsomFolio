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
