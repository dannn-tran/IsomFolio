# IsomFolio Addon Development

Addons are standalone executables that communicate with the host over **NDJSON on stdin/stdout**. They can be written in any language.

## Protocol

All messages are single-line JSON objects (`\n`-terminated). The host sends on **stdin**; the addon sends on **stdout**. Stderr is captured into a ring buffer and shown in crash reports.

### Startup sequence

```
Host                          Addon
  |                             |
  |-- {"id":1,"method":"handshake"} -->|   host spawns process, immediately sends handshake
  |                             |
  |<-- {"type":"ok","id":1,"result":{  |   addon responds immediately (before heavy init)
  |      "protocol_version":1,  |
  |      "addon_version":"1.0.0"|
  |      "capabilities":[...]}} |
  |                             |   <-- addon loads models from disk here
  |<-- {"type":"log",...}       |   (optional progress logs during init)
  |<-- {"type":"ready"}         |   addon is ready to accept requests
  |                             |
  |-- {"id":2,"method":"classify","params":{...}} -->|
  |<-- {"type":"ok","id":2,"result":{...}} |
```

Timeouts (host-enforced):
- **10 s** — handshake response must arrive within 10 s of process spawn. This is a pure liveness check — the addon must not do any heavy work before responding.
- **120 s** — `ready` event must arrive within 120 s of the handshake response. This window is exclusively for model loading from disk. Because liveness is already confirmed by the handshake, the host knows the process is alive and simply waiting for loading to finish.

If either timeout fires, or the protocol version is unsupported, the host kills the process.

### Handshake

Host request:
```json
{"id":1,"method":"handshake"}
```

Addon response:
```json
{"type":"ok","id":1,"result":{"protocol_version":1,"addon_version":"1.0.0","capabilities":["classify"]}}
```

- `protocol_version` — must be `1`.
- `addon_version` — semver string, must match the `version` field in `manifest.json`.
- `capabilities` — array of method names the addon handles (e.g. `["classify"]`).

### Ready event

After the handshake response, the addon loads models from disk. When ready to serve requests it emits:

```json
{"type":"ready"}
```

The host does not send any requests before this event arrives.

### Fatal event

If the addon cannot start (models missing, corrupted files, insufficient memory, unsupported hardware), it emits a `fatal` event instead of `ready`, then exits:

```json
{"type":"fatal","repairable":true,"message":"vision_model.onnx not found — run installer to repair"}
{"type":"fatal","repairable":false,"message":"insufficient memory to load model (requires ~4 GB RAM)"}
```

- `repairable: true` — running the addon's install step will fix the problem. The host offers a "Repair" action.
- `repairable: false` — the installer cannot help. The host surfaces the message with a "Report issue" affordance.

### Request / response loop

**Request:**
```json
{"id":2,"method":"classify","params":{"file_id":"abc123","thumbnail_path":"/path/to/thumb.jpg"}}
```

**Success response:**
```json
{"type":"ok","id":2,"result":{...}}
```

**Error response:**
```json
{"type":"error","id":2,"error":"something went wrong"}
```

`id` is a monotonically increasing integer assigned by the host. Match responses to requests by `id`. Responses may arrive out-of-order.

### Events (addon → host, unsolicited)

```json
{"type":"log","level":"info","message":"loading model..."}
{"type":"progress","id":2,"percent":42}
```

`level` is one of `info`, `warn`, `error`.
`progress` events are per-request; `id` matches the in-flight request id.

### Ping

The host may send `ping` at any time to check liveness:
```json
{"id":3,"method":"ping"}
```
Addon must respond:
```json
{"type":"ok","id":3,"result":{}}
```

---

## Capabilities

| Capability | Method | Params | Result |
|---|---|---|---|
| `classify` | `classify` | `{"file_id":"...","thumbnail_path":"..."}` | `{"file_id":"...","tags":[{"tag":"...","confidence":0.0–1.0},...]}` |
| `cluster_faces` | `cluster_faces` | `{"file_ids":["..."]}` | `{"clusters":[{"label":"...","file_ids":["..."]},...]}` |

Return at most 5 tags for `classify`, in descending confidence order.

---

## Manifest (`manifest.json`)

Place this file alongside the addon binary:

```json
{
  "name": "my-addon",
  "version": "1.0.0",
  "capabilities": ["classify"],
  "has_install_step": true,
  "description": "One-line description shown in Settings.",
  "config_schema": [
    {
      "key": "my_key",
      "label": "Display label",
      "kind": "text",
      "default": "optional default value"
    }
  ]
}
```

`has_install_step` — if `true`, the host invokes `<binary> install --data-dir <path>` immediately after extracting the package, before ever launching the runtime. Omit or set `false` for addons with no model weights (e.g. API-based addons).

`config_schema` is optional. Each field:

| Field | Required | Values |
|---|---|---|
| `key` | yes | identifier written to config JSON |
| `label` | yes | label shown in Settings UI |
| `kind` | no | `text` (default), `secret` (masked input), `select` |
| `default` | no | pre-filled value |
| `options` | only for `select` | array of string options |
| `description` | no | tooltip / hint shown in Settings UI |
| `min` / `max` | no (numeric fields) | inclusive bounds shown in Settings UI |

The host writes user-edited config values to `config.json` in the **same directory as the addon binary**. Read it on startup:

```rust
// Rust — use the isomfolio-extension-sdk helper:
let config: MyConfig = sdk::load_config(&mut out);
```

```csharp
// C#:
var path = Path.Combine(AppContext.BaseDirectory, "config.json");
```

---

## CLI arguments

The host passes the following arguments when invoking the addon binary:

| Mode | Invocation |
|---|---|
| Runtime | `<binary> --data-dir <path>` |
| Install / repair | `<binary> install --data-dir <path>` |

`--data-dir` — directory for storing model weights (persistent across restarts). **Required** in both runtime and install modes. The addon must exit with a clear error if it is absent.

---

## Install mode

When `has_install_step: true`, the host runs the install step immediately after package extraction:

```
autotag-clip install --data-dir /path/to/models
```

The installer must:
1. Download any required model weights into `--data-dir`.
2. Verify the downloaded files (checksum or load test).
3. Emit `{"type":"log",...}` and `{"type":"progress",...}` events for UI feedback.
4. Exit `0` on success, non-zero on failure.

No handshake is performed in install mode — the installer is a short-lived process, not a persistent IPC partner.

The same binary and same `install` subcommand are used for **repair** (re-triggered by the host when the runtime emits `fatal` with `repairable: true`).

### Standalone install (development)

The installer can be run without the host app:

```bash
ADDON_DIR="$HOME/Library/Application Support/IsomFolio/addons/autotag-clip"
autotag-clip install --data-dir "$HOME/Library/Application Support/IsomFolio/models"
```

Output will be raw NDJSON log lines — readable for development purposes.

---

## Packaging

A `.isfx` file is a zip archive with flat contents (no subdirectory):

```
manifest.json
my-addon                <- binary (or my-addon.exe on Windows)
```

Create with:

```bash
cd path/to/addon/build
zip my-addon.isfx manifest.json my-addon
```

---

## Installing

**Via the app:** open Settings (⚙ in the status bar) → **Install from file…** → pick the `.isfx` file. The host:
1. Extracts the archive into the addon directory.
2. Sets the executable bit on the binary.
3. If `has_install_step: true`, runs `<binary> install --data-dir <path>` and streams progress to the UI.
4. Launches the runtime once the install step exits successfully.

**Manually (for development):**

```bash
ADDON_DIR="$HOME/Library/Application Support/IsomFolio/addons/my-addon"
mkdir -p "$ADDON_DIR"
cp manifest.json my-addon "$ADDON_DIR/"
chmod +x "$ADDON_DIR/my-addon"
# If has_install_step: true, run the installer manually first (see above)
```

Restart the app (or re-open a catalog) to discover the addon.

---

## Reference addons

| Addon | Language | `has_install_step` | Description |
|---|---|---|---|
| `autotag-clip/` | Rust | yes | CLIP local inference via `tract-onnx`. Downloads ONNX model weights on first install. |
| `autotag-openai/` | Rust | no | OpenAI Vision API (or any compatible endpoint). Requires `api_key` in config. |
| `../addons-cs/faces/` | C# | yes | InsightFace SCRFD detection + ArcFace recognition, local ONNX inference. |
| `test-addon/` | Rust | no | Minimal echo addon for testing the host protocol. |

---

## Implementation notes

### Startup ordering

The addon **must** respond to the handshake before doing any heavy work. The host has a 10-second window for the handshake response; heavy init happens after it and before `ready`.

Correct order:
1. Read handshake request from stdin.
2. Send handshake response immediately.
3. Load models from disk (emit `log` events for progress).
4. If loading fails: emit `fatal` (with `repairable`) and exit.
5. Send `ready`.
6. Enter the request/response loop.

### Concurrent request handling

The host may pipeline multiple requests without waiting for prior responses (particularly for `classify` via `send_many`). Addons that want to batch-process should read ahead from stdin and group requests. See `autotag-clip` for a batching example using an mpsc channel and timeout-based coalescing.

Addons do not need to handle requests before `ready` — the host guarantees it will not send any.

### Rust SDK helpers

`isomfolio-extension-sdk` provides helpers so addons do not hand-parse arguments:

```rust
let models_dir = sdk::models_dir(&mut out); // parses --data-dir; exits with fatal log if absent
let install_mode = sdk::is_install_mode();  // true when first arg is "install"
```
