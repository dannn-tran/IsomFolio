---
title: Interface Overview
description: A tour of the IsomFolio interface — sidebar, grid, toolbar, and panels.
---

IsomFolio follows a three-column layout: **Sidebar**, **Grid**, and **Info Panel**.

## Sidebar

The left panel lists everything in your library:

| Section | What it shows |
|---|---|
| **All Files** | Every photo across all folders |
| **Folders** | One entry per watched folder. Click to filter to that folder. |
| **Albums** | Manual collections you've created by dragging photos |
| **Smart Albums** | Criteria-based albums that update automatically |
| **People** | Face clusters (if the face-clustering extension is installed) |

Right-click any sidebar item to access actions: rename, delete, remove folder, etc.

## Grid

The main content area shows photos as thumbnails. Key interactions:

- **Click** — select a photo
- **Cmd+Click** — add to selection (multi-select)
- **Click and drag** — drag selected photos to an album in the sidebar
- **Right-click** — open the context menu (add to album, show in Finder, etc.)
- **Cmd+A** — select all
- **Cmd+Shift+A** — deselect all
- **Cmd++** / **Cmd+-** — zoom in / out (adjust thumbnail size)

## Info Panel

Press `I` to toggle the Info panel on the right side. It shows:

- File name, path, dimensions, file size
- Camera metadata: make/model, lens, focal length, aperture, shutter speed, ISO, flash (when EXIF is present)
- GPS location coordinates (when embedded in the photo)
- Tags (add/remove manually or accept/reject AI suggestions)
- Rating (1–5 stars)
- Flag status (unflagged, pick, reject)

When **multiple photos are selected**, the panel switches to batch-edit mode — any tags, ratings, or flags you set apply to all selected photos.

## Toolbar

The toolbar runs along the top:

| Control | Function |
|---|---|
| **Add Folder** | Watch a new folder for photos |
| **Sort** | Cycle through sort fields (name, date, rating) and toggle ascending/descending |
| **Filter** | Open the criteria panel for advanced filtering |
| **View toggle** | Switch between Browse, Preview, and Loupe views |
| **Extension menu** | Run an installed extension (auto-tag, etc.) on selected photos |

## Status Bar

The thin bar at the bottom shows:

- Photo count for the current view
- Thumbnail generation progress (when scanning)
- Extension progress (when auto-tagging)
- Error messages

## View modes

| Mode | How to enter | What it does |
|---|---|---|
| **Browse** | Default | Grid of thumbnails |
| **Preview** | `E` | Single photo, fit to window, with grid still accessible |
| **Loupe** | `Space` | Full-screen single photo with no chrome |
| **Compare** | `C` (with 2 selected) | Side-by-side comparison of two photos |
| **People** | Sidebar → People | Grid of face clusters |
