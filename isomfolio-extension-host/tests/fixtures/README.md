# `isomfolio-extension-host` integration test fixtures

This crate's smoke test (`tests/isfx_package_smoke.rs`) discovers any `.isfx`
files dropped into this directory and runs each one through a real end-to-end
flow: **install → setup → launch → handshake → real inference call → uninstall**.

The fixtures directory is gitignored. Each developer drops in whatever
extension package they want to validate.

## Quick start — testing the bundled Faces extension

```bash
# 1. Publish + package Faces as .isfx (osx-x64 shown; pick the RID for your host)
cd extensions-cs/Faces
dotnet publish -c Release -r osx-x64 --self-contained --nologo

# 2. Zip the publish output's contents and drop it into the fixtures dir
cd bin/Release/net10.0/osx-x64/publish
zip -rq "$OLDPWD/../../../../../../isomfolio-extension-host/tests/fixtures/faces.isfx" . \
    -x "*.pdb" "*.dSYM/*"

# 3. Run the integration test (from the repo root)
cd <repo-root>
cargo test -p isomfolio-extension-host --test isfx_package_smoke
```

For full publish/zip details — runtime identifiers, AOT trade-offs, layout — see
`extensions-cs/README.md`. **Architecture must match**: `osx-arm64` for Apple
Silicon, `osx-x64` for Intel Macs, otherwise launch fails with a cryptic
"type initializer threw an exception".

## Faster iteration with `buffalo_s`

The Faces extension reads `ISFX_FACES_VARIANT` at both setup (which zip to
download) and runtime (which model files to load):

- unset or `buffalo_l` — full models, ~280 MB
- `buffalo_s` — small models, ~25 MB

For tight test loops:

```bash
ISFX_FACES_VARIANT=buffalo_s cargo test -p isomfolio-extension-host --test isfx_package_smoke
```

Set the env var both when running the test (the setup subprocess inherits it)
and at any later launch.

## What the test asserts per capability

Looks at the manifest's declared capabilities and dispatches:

| Capability | Behaviour |
|---|---|
| `cluster_faces` | Sends `extensions-cs/Faces.Tests/Assets/test_face.jpg` via `cluster_faces`, asserts at least one face is detected (`clusters` + `noise` ≥ 1). 10-min ceiling; stderr from the extension is dumped on failure. |
| `classify` | Calls `classify` on the same image, asserts the response has a `tags` field. |
| anything else | Logged as "no inference smoke test wired for capability '<name>'" — extend `exercise_capability` in the test file to add coverage. |

All capabilities also go through a generic `ping` round-trip; extensions that
don't implement ping are allowed to return an "unknown method" error.

## Skip vs. require

If no `.isfx` is present, the test passes with a notice. Set
`ISFX_REQUIRE_PACKAGE=1` to make absence fail the test — useful on CI when a
package is expected:

```bash
ISFX_REQUIRE_PACKAGE=1 cargo test -p isomfolio-extension-host --test isfx_package_smoke
```

## When it fails

The test panics with the extension's last stderr output. Common patterns:

- Empty stderr + "extension exited" mid-flow → suspect a native crash, OS OOM
  kill, or unhandled exception that didn't flush. Re-run with `cargo test …
  -- --nocapture` to see the full conversation including the host's reader logs.
- "type initializer threw an exception" → architecture mismatch between the
  binary and the host.
- "missing field `extension_version`" → handshake response shape regression
  (C# side serialises `ExtensionVersion` with the snake_case naming policy).
