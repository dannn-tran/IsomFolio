---
title: Browsing Photos
description: Navigating your library, viewing photos, and using the loupe and compare views.
---

import { Aside } from '@astrojs/starlight/components';

## Navigating the grid

Use the **sidebar** to switch between All Files, a specific folder, an album, or a person. The grid updates immediately.

Sort options (accessible from the toolbar or sort button):
- **Name** — alphabetical by filename
- **Date** — EXIF capture date (`DateTimeOriginal`) when available; falls back to file modification date
- **Rating** — highest first
- Toggle ascending / descending with the sort direction button

## Loupe view

Press `Space` (or double-click a photo) to enter **Loupe** — a full-screen view of the selected photo.

In Loupe:
- `←` / `→` — navigate to the previous / next photo
- `Space` or `Esc` — return to the grid (lands on the same photo in the grid)
- `P` / `X` / `U` — set flag (Pick / Reject / Unflagged)
- `1`–`5` — set rating
- The Info panel (`I`) remains available on the right

**Zoom in to inspect detail:** scroll or use a two-finger trackpad gesture over the photo to zoom toward the pointer, or use the **− / + / Fit** buttons at the bottom. When zoomed in, drag the photo to pan. Zoom resets to fit each time you move to another photo.

For RAW files, the fit view uses the camera's fast embedded preview so browsing stays instant; the slower full decode is done only when you zoom in, so a 100% focus check is still pixel-accurate.

<Aside type="tip">
Loupe pre-fetches adjacent photos in the background so navigation is instant even for large files.
</Aside>

## Preview mode

Press `E` to enter **Preview** — a single-photo view that keeps the sidebar and status bar visible. Useful when you want full-resolution context without going fully full-screen.

## Compare mode

Select exactly **two photos** in the grid, then press `C`. Both photos display side by side at full resolution. Use this to decide between two similar shots.

Press `Esc` to return to the grid.

## Thumbnail zoom

- `Cmd++` — increase thumbnail size (up to 400 px)
- `Cmd+-` — decrease thumbnail size (down to 80 px)

The zoom level is preserved between sessions.

## Show in Finder

Right-click any photo and choose **Show in Finder** to reveal the original file. Use this to open the photo in an external editor.

## Hiding rejects

Press `\` (backslash) to toggle visibility of rejected photos. When hidden, photos flagged as **Reject** disappear from the grid. This is useful after a first-pass cull — flag your rejects, then hide them to focus on what's left.
