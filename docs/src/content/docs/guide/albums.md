---
title: Albums & Smart Albums
description: Organise photos into manual collections or criteria-driven smart albums.
---

import { Aside } from '@astrojs/starlight/components';

IsomFolio has two types of albums: **manual albums** you curate by hand, and **smart albums** that populate themselves based on criteria.

## Manual albums

Manual albums are collections you assemble by dragging photos into them.

### Create an album

Click the `+` button in the **Albums** section header and choose **New Album**. The name field grabs focus automatically — type a name and press `Enter`. The new album appears in the sidebar without pulling you away from your current view, so you can keep culling and fill it later.

### Add photos to an album

- **Drag and drop** — select photos in the grid and drag them to an album in the sidebar. A highlight appears on the target album when you hover over it.
- **Context menu** — right-click selected photos and choose **Add to Album →**, then pick the album from the submenu.

### Remove photos from an album

Right-click selected photos and choose **Remove from Album**. This removes them from the album without deleting the original files.

### Rename or delete an album

Right-click the album in the sidebar to access rename and delete options. Deleting an album removes the collection definition — the original photos are not affected.

### Duplicate an album

Right-click an album and choose **Duplicate**. Useful as a starting point for a related collection.

### Copy an album to a folder

Right-click an album and choose **Copy to Folder…**, then pick a destination. IsomFolio creates a **sub-folder named after the album** inside the destination and copies every photo currently in the album into it, leaving the originals untouched — no processing or re-encoding is applied. This works for both manual and smart albums — for a smart album it copies whatever matches its criteria at that moment. Offline (missing) files are skipped.

Copies are **non-destructive**: an existing folder is merged into rather than replaced, and if a file of the same name is already there it's kept — the incoming copy gets a numeric suffix instead (`photo.jpg` → `photo (1).jpg`).

## Shelves

When you have lots of albums, group them onto **shelves**. A shelf is a named container that holds albums — think of it as a bookshelf your photo albums sit on (for example, a shelf per year or per client).

### Create a shelf

Click the `+` button in the **Albums** section header and choose **New Shelf**, type a name, and press `Enter`. The shelf appears in the sidebar with a disclosure chevron; click its header to collapse or expand the albums beneath it.

### File an album under a shelf

Right-click an album and choose **Move to Shelf →**, then pick a shelf — or **Ungrouped** to lift it back to the top of the list. Both manual and smart albums can live on a shelf. Duplicating an album keeps it on the same shelf as the original.

### Copy a shelf to a folder

Right-click (or **Ctrl+Click**) a shelf header and choose **Copy to Folder…**, then pick a destination. IsomFolio mirrors the shelf's structure on disk — it creates a folder named after the shelf, and inside it one sub-folder per album, each holding that album's photos (`<destination>/<shelf>/<album>/…`). This is the way to copy several albums at once while keeping them organised. The same non-destructive rules apply: existing folders are merged into, and colliding filenames get a numeric suffix.

### Rename or delete a shelf

Right-click (or **Ctrl+Click**) a shelf header for **Rename** and **Delete Shelf**, then confirm. Deleting a shelf only removes the container — its albums are kept and simply become ungrouped again. Your photos are never affected.

## Smart albums

Smart albums automatically include every photo that matches a set of criteria. They update in real time as you add photos, edit tags, or change ratings.

### Create a smart album

1. Open the criteria panel (filter toggle in the toolbar).
2. Set your criteria — tags, rating, flag, date range, file types.
3. Click **Save as Smart Album** at the bottom of the criteria panel.
4. Enter a name.

The new smart album appears in the sidebar and stays in sync automatically.

### Edit a smart album

Click a smart album in the sidebar. Its criteria appear in the criteria panel. Modify them and click **Update Smart Album** to save the changes.

<Aside type="tip">
Smart albums are non-destructive. Changing the criteria doesn't move or affect your original photos — it only changes which photos appear in the album view.
</Aside>

### Dirty indicator

When you modify criteria while viewing a smart album without saving, a dot indicator appears next to the album name in the sidebar to signal unsaved changes.

## Typical workflows

**Portfolio selection**
1. Tag your best work with a keyword like `portrait`.
2. Create a smart album: rating ≥ 4 stars AND tag = `portrait`.
3. The album shows your best portraits, auto-updating as you rate more photos.

**Shoot delivery**
1. Cull a shoot by flagging picks and rejects.
2. Create a smart album: flag = Pick AND folder = "Client Shoot 2024".
3. Drag the album contents to a new manual album for the final delivery set.

**Rapid event tagging**
1. Sync a folder from an event.
2. Select all, add a tag `event/wedding-2024-06` in the Info panel.
3. A smart album on that tag captures everything from the event.
