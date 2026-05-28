---
title: Albums & Smart Albums
description: Organise photos into manual collections or criteria-driven smart albums.
---

import { Aside } from '@astrojs/starlight/components';

IsomFolio has two types of albums: **manual albums** you curate by hand, and **smart albums** that populate themselves based on criteria.

## Manual albums

Manual albums are collections you assemble by dragging photos into them.

### Create an album

Right-click in the Albums section of the sidebar and choose **New Album**, or use the `+` button that appears when hovering the section header. Enter a name and press `Enter`.

### Add photos to an album

- **Drag and drop** — select photos in the grid and drag them to an album in the sidebar. A highlight appears on the target album when you hover over it.
- **Context menu** — right-click selected photos and choose **Add to Album →**, then pick the album from the submenu.

### Remove photos from an album

Right-click selected photos and choose **Remove from Album**. This removes them from the album without deleting the original files.

### Rename or delete an album

Right-click the album in the sidebar to access rename and delete options. Deleting an album removes the collection definition — the original photos are not affected.

### Duplicate an album

Right-click an album and choose **Duplicate**. Useful as a starting point for a related collection.

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
1. Auto-tag all photos with the CLIP extension.
2. Create a smart album: rating ≥ 4 stars AND tag = `portrait`.
3. The album shows your best portraits, auto-updating as you rate more photos.

**Shoot delivery**
1. Cull a shoot by flagging picks and rejects.
2. Create a smart album: flag = Pick AND folder = "Client Shoot 2024".
3. Drag the album contents to a new manual album for the final delivery set.

**Rapid event tagging**
1. Scan a folder from an event.
2. Select all, add a tag `event/wedding-2024-06` in the Info panel.
3. A smart album on that tag captures everything from the event.
