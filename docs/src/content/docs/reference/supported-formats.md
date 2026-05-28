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
| XMP | `.xmp` | Tracked alongside paired images; content is not parsed by IsomFolio |

XMP sidecar files are tracked in the database alongside their paired image. Changes to a `.xmp` file trigger a rescan of the associated image's metadata. IsomFolio does not currently read or write XMP metadata — it tracks sidecar presence for future use.

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
