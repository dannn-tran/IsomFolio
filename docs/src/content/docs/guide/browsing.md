---
title: Browsing Photos
description: Navigating your library, viewing photos, and using the loupe and compare views.
---

import { Aside } from '@astrojs/starlight/components';

## Navigating the grid

Use the **sidebar** to switch between **All Photos**, a specific folder, an album, or a person. The grid updates immediately. **All Photos** sits at the top of the sidebar and is your home view — click it any time to return to the whole catalog.

**Imports.** Each sync that adds new photos is recorded as a dated **import batch**, listed under **Imports** in the sidebar (`Jun 3 (80)` = 80 photos imported that day). Click one to see exactly the photos that came in during that sync — a fixed set that never changes, so "show me what I just brought in" is always one click. The ten most recent are shown; **Show all** expands the full history. This is distinct from the **Added** filter (Searching & Filtering), which is a rolling time window you can combine with other criteria.

Sort options (accessible from the toolbar or sort button):
- **Name** — alphabetical by filename
- **Date** — EXIF capture date (`DateTimeOriginal`) when available; falls back to file modification date
- **Rating** — highest first
- Toggle ascending / descending with the sort direction button

## Grid and List views

Two toolbar buttons switch how the content area lays out photos:

- **▦ Grid** (default) — a thumbnail grid. Use the **− / +** buttons in the toolbar (or `Cmd++` / `Cmd+−`) to change thumbnail size.
- **≡ List** — a compact line per photo with columns: thumbnail, **Name**, flag, rating, colour label, **Date**, **Size**, and **Type**. Best for scanning filenames and metadata at a glance, like the list view in Finder or Capture One.

In List view, **click a column header** (Name / Date / Size / Type) to sort by it; click the active column again to flip the direction (`▲` / `▼`). **Drag a column's right edge** to resize it (Name, Rating, Date, Size, Type) — widths are remembered for the session. Everything else — selection, filtering, the cull strip, the Info panel, drag-to-album, and right-click menus — works exactly the same in both views.

## Loupe view

Press `Space` (or double-click a photo) to enter **Loupe** — a full-screen view of the selected photo.

In Loupe:
- `←` / `→` — navigate to the previous / next photo
- `Space` or `Esc` — return to the grid (lands on the same photo in the grid)
- `P` / `X` / `U` — set flag (Pick / Reject / Unflagged)
- `1`–`5` — set rating
- The Info panel (`I`) remains available on the right

**Zoom in to inspect detail:** scroll or use a two-finger trackpad gesture over the photo to zoom toward the pointer, **click the photo** to jump to 1:1 at the spot you clicked (click again to return to Fit), press the **`+`** / **`−`** keys, or use the magnifier **zoom-out / zoom-in / 1:1 / Fit** buttons at the bottom. **`Z`** toggles between Fit and **1:1** (actual pixels) for a precise focus check. When zoomed in, drag the photo to pan. Zoom resets to fit each time you move to another photo — unless you enable the **🔒 lock** button, which keeps the zoom and pan position as you move through photos (ideal for checking focus on the same spot across a burst). The **⛶** button toggles fullscreen.

For RAW files, the fit view uses the camera's fast embedded preview so browsing stays instant; the slower full decode is done only when you zoom in, so a 100% focus check is still pixel-accurate.

<Aside type="caution" title="“Can’t open this photo” in the loupe">
If the loupe shows a card saying it can’t open the photo, IsomFolio could read the cached thumbnail (so the grid looks fine) but not the original file. On macOS this is almost always a **privacy permission**: photos in `~/Downloads`, `~/Desktop`, or `~/Documents` are protected folders. Click **Open Privacy Settings** on the card and grant IsomFolio **Full Disk Access**, then reopen the photo — or move your library to an unprotected folder.
</Aside>

A **filmstrip** of neighbouring photos runs under the image — the current photo is ringed; click any thumbnail to jump to it.

<Aside type="tip">
Loupe pre-fetches adjacent photos in the background so navigation is instant even for large files.
</Aside>

## Preview mode

Press `E` to enter **Preview** — a single-photo view that keeps the sidebar and status bar visible. Useful when you want full-resolution context without going fully full-screen.

## Review mode

Select **two or more photos** in the grid, then press `C` (or `Space`). They open in the **review surface** — side by side — one place with two layouts you flip between with `Space`:

- **Survey** (the default) — all frames side by side, fit to the window, switchable between a horizontal **Row** and a **Grid** (no scrolling), with **synced zoom** (zoom one and every frame tracks the same spot for a 100% comparison).
- **One-up** — the focused frame big over a filmstrip; turn on **Lock zoom** for a blink comparison as you step between frames.

The **sharpest** frame is marked **◉ Sharpest** and each shows its rank (**Sharp #2 / 5**) — a *relative* comparison among the frames on screen, not an absolute "in focus" verdict. Flag the keeper with `P` and the rest with `X`, press `R` to drop an also-ran from the set, and `Esc` to return to the grid. Covered in detail under [Culling → Comparing similar shots](/guide/culling/).

## Thumbnail zoom

- **− / +** toolbar buttons, or `Cmd++` / `Cmd+−` — change thumbnail size (80–400 px).

The zoom level is preserved between sessions.

## Working offline (removable drives)

You can keep browsing and culling from thumbnails even when a photo's drive is disconnected:

- A library root on an unplugged drive is marked **offline** in the sidebar (`⏏`); its photos still appear in the grid with an **Offline** badge.
- **Flagging, rating, colour labels, and rejects all work offline** — they're saved to the catalog and apply to the originals next time the drive is connected.

Opening a photo in the loupe (and full-resolution zoom and export) needs the original file, so reconnect the drive for those. When you reconnect, IsomFolio notices automatically and the offline markers clear.

## Show in Finder

Right-click any photo and choose **Show in Finder** to reveal the original file. Use this to open the photo in an external editor.

## Hiding rejects

Press `\` (backslash) to toggle visibility of rejected photos. When hidden, photos flagged as **Reject** disappear from the grid. This is useful after a first-pass cull — flag your rejects, then hide them to focus on what's left.
