---
title: Catalog Format
description: Technical reference for the IsomFolio catalog directory structure and database schema.
---

A **catalog** is a directory with the `.isfcatalog` extension. It is self-contained — you can move it, copy it, back it up, or share it as a single directory.

## Directory layout

```
MyPhotos.isfcatalog/
├── catalog.db          ← SQLite database
└── thumbnails/         ← JPEG thumbnail cache
    ├── abc123.jpg
    └── def456.jpg
```

## catalog.db

The database is a standard SQLite 3 file. You can open and query it with any SQLite client.

### Key tables

**files** — one row per tracked photo

| Column | Type | Description |
|---|---|---|
| `id` | TEXT | SHA-256 of the normalised file path (primary key) |
| `path` | TEXT | Absolute path to the original file |
| `folder_path` | TEXT | Parent folder path |
| `filename` | TEXT | File name |
| `ext` | TEXT | File extension (lowercase) |
| `size_bytes` | INTEGER | File size |
| `mtime_unix` | INTEGER | File modification time (Unix timestamp) |
| `rating` | INTEGER | 0–5 (0 = no rating) |
| `flag` | INTEGER | 0 = unflagged, 1 = pick, -1 = reject |
| `fts_dirty` | INTEGER | Full-text search rebuild pending flag |

**tags** — many-to-many: files ↔ tags

| Column | Type | Description |
|---|---|---|
| `file_id` | TEXT | References `files.id` |
| `tag` | TEXT | Tag string |
| `confidence` | REAL | Optional; null for manual and imported tags |

**albums** — album definitions

| Column | Type | Description |
|---|---|---|
| `id` | TEXT | UUID |
| `name` | TEXT | Display name |
| `kind` | TEXT | `manual` or `smart` |
| `criteria_json` | TEXT | JSON criteria for smart albums |

**album_files** — many-to-many: manual albums ↔ files

**face_clusters** / **face_cluster_members** — face recognition data

**folders** — watched folder paths

## Thumbnail cache

Thumbnails are JPEG files named by file ID. They are generated on demand and cached indefinitely. The cache can be safely deleted — thumbnails are regenerated on next launch (with a performance cost).

To clear the thumbnail cache:

```sh
rm -rf MyPhotos.isfcatalog/thumbnails/*
```

## Migrations

The database schema is managed by a migration system. Migrations are forward-only — each new version of IsomFolio adds migrations if needed and runs them on startup. There is no downgrade path.

## App data (not in the catalog)

Items stored in `~/Library/Application Support/IsomFolio/`:

| File | Purpose |
|---|---|
| `recent_catalogs.json` | List of recently opened catalog paths |
| `settings.json` | App preferences (auto-detect people, inference engine, import options, etc.) |
| `extensions/` | Installed extension directories |
| `crash_reports/` | Extension crash reports |
| `face_crops/` | Cached face crop images for the People view |
