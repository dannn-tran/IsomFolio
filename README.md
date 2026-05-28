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

## Project structure

```
isomfolio-core/   # Library: DB, indexing, scanning, thumbnails, search
isomfolio-app/    # Binary: iced UI (app.rs + view.rs)
docs/             # Astro + Starlight documentation site
dev-docs/         # Internal design docs and engineering notes
```

## Supported image formats

JPEG, PNG, WebP, GIF. XMP sidecar files (`.xmp`) are also tracked alongside their paired images.
