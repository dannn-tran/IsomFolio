---
title: Supported File Formats
description: Image formats and sidecar files that IsomFolio can index and display.
---

## Image formats

| Format | Extension | Notes |
|---|---|---|
| JPEG | `.jpg`, `.jpeg` | Full support |
| PNG | `.png` | Full support |
| WebP | `.webp` | Full support |
| GIF | `.gif` | First frame used for thumbnail |

## Sidecar files

| Format | Extension | Notes |
|---|---|---|
| XMP | `.xmp` | Sidecar read on sync — keywords, rating, and label imported |

When a `.xmp` sidecar is present, IsomFolio reads it on every sync:

- **`dc:subject` keywords** are imported as tags, merged with your manual tags (imported and manual tags are not distinguished once in the catalog)
- **`xmp:Rating`** seeds the photo's star rating
- **`xmp:Label`** and Dublin Core title/description are shown read-only in the Info panel

When the file watcher detects that a `.xmp` sidecar has changed, a notification appears in the sidebar — **"XMP updated — N files"** — with Apply and Dismiss buttons. Clicking Apply imports the new metadata; Dismiss ignores the change until the next manual sync. XMP writing is not supported — IsomFolio does not modify your sidecar files.

## Formats not supported

The following formats are **not** currently indexed or displayed:

| Format | Notes |
|---|---|
| RAW files (`.cr2`, `.nef`, `.arw`, `.dng`, etc.) | Planned; requires a RAW decode library |
| HEIC / HEIF | Planned for macOS builds (system decoder available) |
| TIFF | Not currently indexed |
| Video files | Not applicable — IsomFolio is a photo manager |

## Adding RAW support (planned)

RAW format support is on the roadmap. The planned approach is to use the system RAW decoder on macOS (via `ImageIO`) for thumbnail generation, keeping the dependency optional for Linux/Windows builds.
