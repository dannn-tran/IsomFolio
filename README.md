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

Evaluate **grouping quality** — whether the embedding-based scene grouper beats the cheap phash burst grouper — against a labelled test set (one subfolder per ground-truth "shot"):

```sh
cargo run --release -p isomfolio-core --bin bench-grouping -- /path/to/testset
cargo run --release -p isomfolio-core --bin bench-grouping -- /path/to/testset --limit 500
```

For each grouper it sweeps the relevant parameter (burst Hamming threshold; scene `eps` × `min_pts`) and scores it against the folder labels with pairwise Precision/Recall/F1 and the Adjusted Rand Index, then counts how many "recomposed" same-group pairs (too far apart in Hamming for bursts to link) each one captures — the direct measure of whether scenes add utility. Pure `isomfolio-core`: both groupers are pure functions and the scene descriptor is model-free, so no inference engine or database is involved.

## Project structure

```
isomfolio-core/   # Library: DB, indexing, scanning, thumbnails, search
isomfolio-app/    # Binary: iced UI (app.rs + view.rs)
docs/             # Astro + Starlight documentation site
dev-docs/         # Internal design docs and engineering notes
```

## Supported image formats

JPEG, PNG, WebP, GIF. XMP sidecar files (`.xmp`) are also tracked alongside their paired images.
