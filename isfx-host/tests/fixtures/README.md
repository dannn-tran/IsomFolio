# Integration test fixtures

Drop `.isfx` packages here and `cargo test -p isfx-host` will smoke-test each one
end-to-end (install → launch → ping → uninstall).

```bash
cp path/to/your-extension.isfx isfx-host/tests/fixtures/
cargo test -p isfx-host
```

`.isfx` files are gitignored — these are local artifacts for the developer who's
testing them.

## Faster model variant for Faces

The Faces extension reads `ISFX_FACES_VARIANT` at runtime:

- unset or `buffalo_l` — full models (~280 MB, higher accuracy)
- `buffalo_s` — small models (~25 MB, faster download and inference)

For test iteration:

```bash
ISFX_FACES_VARIANT=buffalo_s cargo test -p isfx-host
```

The variant selection affects both setup (which zip the extension downloads) and
runtime (which file names it loads), so the env var must be present for both
phases.

## Forcing a fixture to be present

To fail (instead of skip) when no `.isfx` is in this directory — e.g. on CI:

```bash
ISFX_REQUIRE_PACKAGE=1 cargo test -p isfx-host
```
