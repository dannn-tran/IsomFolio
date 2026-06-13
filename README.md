# IsomFolio

Photo library manager for macOS. Organizes images into catalogs with tagging, smart albums, and live folder watching.

## Requirements

- Rust 1.80+ (`rustup show`)
- macOS 12+ (Linux builds work but are untested)

## Build & run

```sh
# Debug build
cargo run -p isomfolio-app

# Pass a catalog directory explicitly (created on first run)
cargo run -p isomfolio-app -- /path/to/MyPhotos.isfcatalog

# Release build
cargo build --release -p isomfolio-app
./target/release/isomfolio-app /path/to/MyPhotos.isfcatalog
```

If no catalog path is given, the app defaults to `./IsomFolio-Catalog.isfcatalog` in the working directory.

## Catalog format

A catalog is a directory with a `.isfcatalog` extension containing:

```
MyPhotos.isfcatalog/
├── catalog.db       # SQLite database (auto-created)
└── thumbnails/      # JPEG thumbnail cache (auto-created)
```

Recent catalogs and session state are stored in `~/Library/Application Support/IsomFolio/`.

## Tests

```sh
cargo test
```

Tests in `isomfolio-core` use temporary SQLite databases for isolation. No fixtures or external services needed.

## Benchmarks

Profile thumbnail generation against a real photo folder (the folder is a CLI argument, so no private path is committed):

```sh
# Always --release — a debug decode is several times slower and misleading.
cargo run --release -p isomfolio-core --bin bench-thumbnails -- /path/to/photos

# Quick run on a subset, or pin a single thread count instead of the sweep
cargo run --release -p isomfolio-core --bin bench-thumbnails -- /path/to/photos --limit 200
cargo run --release -p isomfolio-core --bin bench-thumbnails -- /path/to/photos --concurrency 4
```

It reports a per-decode-path time breakdown (JPEG fast path vs RAW preview/full demosaic, decode vs resize) and a worker-thread concurrency sweep, writing thumbnails to a throwaway temp dir. Use it to find where time goes and whether more threads help.

## Project structure

```
isomfolio-core/   # Library: DB, indexing, scanning, thumbnails, search
isomfolio-app/    # Binary: iced UI (app.rs + view.rs)
docs/             # Astro + Starlight documentation site
dev-docs/         # Internal design docs and engineering notes
```

## Supported image formats

JPEG, PNG, WebP, GIF. XMP sidecar files (`.xmp`) are also tracked alongside their paired images.
