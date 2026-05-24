# IsomFolio Addon Development

Addons are standalone executables that communicate with the host over **NDJSON on stdin/stdout**. They can be written in any language.

## Protocol

### Startup

The host spawns the addon and waits up to 120 seconds for a hello message on stdout:

```json
{"type":"hello","protocol_version":1,"addon_api_version":1,"capabilities":["classify"]}
```

Fields:
- `protocol_version` — must be `1`
- `addon_api_version` — must be `1`
- `capabilities` — array of method names the addon handles (e.g. `["classify"]`)

If the hello is not received in time, or the protocol version is wrong, the host kills the process.

### Request / response loop

After the hello, the host sends requests on stdin and reads responses from stdout. Each message is one JSON object per line.

**Request:**
```json
{"id":1,"method":"classify","params":{"file_id":"abc123","thumbnail_path":"/path/to/thumb.jpg"}}
```

**Success response:**
```json
{"id":1,"result":{...}}
```

**Error response:**
```json
{"id":1,"error":"something went wrong"}
```

`id` is a monotonically increasing integer. Match responses to requests by `id`.

### Events (addon → host, unsolicited)

```json
{"type":"log","level":"info","message":"downloading model..."}
{"type":"progress","id":1,"percent":42}
```

`level` is one of `info`, `warn`, `error`. Log events go to the host's log output.
`progress` events are per-request; `id` matches the in-flight request id.

### Capabilities

| Capability | Method | Params | Result |
|---|---|---|---|
| `classify` | `classify` | `{"file_id":"...","thumbnail_path":"..."}` | `{"tags":[{"tag":"...","confidence":0.0–1.0},...]}` |

Return at most 5 tags in descending confidence order.

---

## Manifest (`isomfolio-addon.json`)

Place this file alongside the addon binary:

```json
{
  "name": "my-addon",
  "protocol_version": 1,
  "addon_api_version": 1,
  "capabilities": ["classify"],
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

`config_schema` is optional. Each field:

| Field | Required | Values |
|---|---|---|
| `key` | yes | identifier written to config JSON |
| `label` | yes | label shown in Settings |
| `kind` | no | `text` (default), `secret` (masked input), `select` |
| `default` | no | pre-filled value |
| `options` | only for `select` | array of string options |

The host writes user-edited values to a config file and passes its path via `ISOMFOLIO_ADDON_CONFIG`. Read it on startup:

```rust
let path = std::env::var("ISOMFOLIO_ADDON_CONFIG").unwrap_or_default();
let config: MyConfig = std::fs::read_to_string(&path)
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_default();
```

---

## Environment variables

| Variable | Value |
|---|---|
| `ISOMFOLIO_MODELS_DIR` | Directory for storing model weights (persistent across restarts) |
| `ISOMFOLIO_ADDON_CONFIG` | Path to this addon's config JSON file |

---

## Building

Use any language. Rust example (see `isomfolio-autotag-clip/` and `isomfolio-autotag-openai/` for full working addons):

```bash
cargo build --release -p my-addon
```

The binary name **must match the addon's `name` field** in `isomfolio-addon.json`.

---

## Packaging

A `.faddon` file is a zip archive with flat contents (no subdirectory):

```
isomfolio-addon.json
my-addon                <- binary (or my-addon.exe on Windows)
```

Create with:

```bash
cd path/to/addon
zip my-addon.faddon isomfolio-addon.json my-addon
```

---

## Installing

**Via the app:** open Settings (⚙ in the status bar) → **Install from file…** → pick the `.faddon` file. The app extracts it, sets the executable bit, and launches the addon immediately.

**Manually (for development):**

```bash
ADDON_DIR="$HOME/Library/Application Support/IsomFolio/addons/my-addon"
mkdir -p "$ADDON_DIR"
cp isomfolio-addon.json my-addon "$ADDON_DIR/"
chmod +x "$ADDON_DIR/my-addon"
```

Restart the app (or re-open a catalog) to discover the addon.

---

## Reference addons

| Addon | Description |
|---|---|
| `isomfolio-autotag-clip/` | CLIP local inference via `tract-onnx`. Downloads model weights on first run. Produces binary `autotag-clip`. |
| `isomfolio-autotag-openai/` | OpenAI Vision API (or any compatible endpoint). Requires `api_key` in config. Produces binary `autotag-openai`. |
| `isomfolio-test-addon/` | Minimal echo addon for testing the host protocol. Produces binary `test-addon`. |
