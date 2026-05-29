# IsomFolio C# Extensions

C#/.NET implementations of IsomFolio extensions.

## Projects

- **`Sdk/`** — shared protocol library (`IsomFolio.Extensions.Sdk`). Message types, JSON serialization, stdin/stdout reader/writer. Targeted by every extension.
- **`Sdk.Tests/`** — xUnit tests for the SDK.
- **`Faces/`** — face detection + clustering extension using InsightFace ONNX models. Capability: `cluster_faces`.
- **`Faces.Tests/`** — integration tests for Faces (require ONNX models on disk).

## Prerequisites

- .NET 10 SDK
- macOS or Linux for builds (Windows untested)
- ~600 MB free disk for ONNX model download during setup

## Building and packaging an extension

Extensions are distributed as `.isfx` files — zip archives containing the extension's published binary, manifest, native libs, and any auxiliary data.

The example below packages **Faces** for Apple Silicon. Adjust the runtime ID (`-r`) for your target: `osx-arm64`, `osx-x64`, `linux-x64`, `linux-arm64`, `win-x64`.

### 1. Publish

```bash
cd extensions-cs/Faces
dotnet publish -c Release -r osx-arm64 --self-contained
```

Output lands in `bin/Release/net10.0/osx-arm64/publish/`. This includes:

- `faces` — the extension executable
- `manifest.json` — declares name, version, capabilities, config schema
- `libonnxruntime.dylib` and other native libs
- `runtimes/` subdir (when not AOT)

### 2. Package

The installer expects the archive's **root** to contain `manifest.json` plus the executable named after the manifest's `name` field. Zip the publish dir's contents — not the dir itself.

```bash
cd bin/Release/net10.0/osx-arm64/publish
zip -r ../../../../../../dist/faces.isfx . -x "*.pdb"
```

- `.` zips the current directory's contents at the archive root
- `-x "*.pdb"` excludes debug symbols (large, not needed at runtime). Stack multiple patterns to drop more: `-x "*.pdb" "*.xml"`. Omit `-x` to include everything

The installer preserves directory structure, so nested layouts (e.g. `runtimes/osx-arm64/native/…`) are extracted intact.

### 3. Install

In IsomFolio: **Settings → Extensions → Install Extension…** and pick the `.isfx` file.

On install, IsomFolio:
1. Validates `manifest.json`
2. Extracts under `~/Library/Application Support/IsomFolio/extensions/<name>/` (macOS) — equivalent paths on Linux/Windows
3. Marks the executable executable (`chmod +x` on Unix)
4. Runs the extension's setup step if `needs_setup: true` in the manifest (downloads model weights for Faces)
5. Launches and performs the handshake

## Architecture matters

If your `.isfx` ships a binary for one architecture but the host process is another, ONNX Runtime fails with cryptic errors like `A type initializer threw an exception`. Build for the user's actual architecture:

- Apple Silicon Mac → `osx-arm64`
- Intel Mac → `osx-x64`

To support both, build twice and ship two `.isfx` files.

## AOT publish

Faces sets `<PublishAot>true</PublishAot>` for fast startup. AOT bundles the native runtime libs into a single binary directory, but the .NET trim analyzer can't always trace what the ONNX wrapper P/Invokes — startup may fail on some configurations.

If AOT misbehaves, set `<PublishAot>false</PublishAot>` in `Faces.csproj` and publish again. Slower startup, but the standard runtime layout under `runtimes/` is preserved and more reliable.

## Running tests

```bash
dotnet test                                    # all
dotnet test Sdk.Tests/Sdk.Tests.csproj         # SDK only
dotnet test Faces.Tests/Faces.Tests.csproj     # Faces (downloads ONNX models on first run, slow)
```

## Protocol

Extensions communicate with the host over newline-delimited JSON on stdin/stdout. See `Sdk/InboundMessage.cs`, `Sdk/ResponseTypes.cs`, and the Rust side at `isomfolio-core/src/extension/protocol.rs` for the wire format.

The handshake response **must** include `extension_version` (snake_case). The `Sdk.HandshakeResult` record is named `ExtensionVersion` — the snake_case JSON naming policy on `SdkJsonContext` produces the correct field name.
