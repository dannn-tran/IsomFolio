---
title: Tagging
description: Add, manage, and search tags.
---

Tags are freeform text labels attached to photos. They power smart albums and search.

## Adding tags manually

1. Select one or more photos.
2. Open the Info panel (`I`).
3. Type in the tag input field. Autocomplete suggests existing tags as you type.
4. Press `Enter` or `Tab` to add the tag.
5. Click the `×` next to a tag to remove it.

All tag edits are undoable (`Cmd+Z`).

## Repeat last tag

Press `.` (period) to apply the last tag you used to the currently selected photo(s). This is useful when tagging multiple photos with the same tag without opening the Info panel each time.

## Batch tagging

Select multiple photos before opening the Info panel. The panel enters **batch mode**:
- Tags shown are common to all selected photos
- Adding a tag adds it to all selected photos
- Removing a tag removes it from all selected photos

## Tag hierarchies

Tags support a path-style hierarchy using `/` as a separator:

```
subject/portrait
subject/landscape
location/paris
gear/sony-a7r5
```

The **Tag Browser** (accessible from the View menu) shows your full tag tree, lets you rename tags across all photos, and lets you delete tags entirely.

### Renaming a tag

Renaming a tag in the Tag Browser updates every photo that has that tag — it's a global rename, not a per-photo edit.

## Where tags come from

Tags are added by you in the Info panel, or imported from existing photo metadata the first time a photo is indexed — XMP keywords (`dc:subject`) and, on macOS, Apple Finder tags. Keyword import is on by default; toggle it under **Settings → General** ("Import XMP keywords" / "Import Apple Finder tags"). The toggle is **forward-only** — turning it off stops importing keywords for newly-indexed photos but never removes tags already imported. Imported tags are merged in additively and are never removed on re-sync.

## Tag browser

Open the Tag Browser from the View menu to:
- See all tags used across your library with usage counts
- Search and filter tags
- Rename a tag (updates all photos)
- Delete a tag (removes it from all photos)

## Descriptive metadata

Below the tags in the Info panel are editable **Title**, **Caption**, **Creator**, and **Copyright** fields (Dublin Core / IPTC). Type a value and press **Enter** to save. With multiple photos selected, the fields start blank and saving applies to **all** of them — handy for stamping a rights or creator statement across a selection. These fields are full-text searchable.

## Saving metadata to files & exporting

Your catalog is the working store, but you can make metadata portable:

- **Photo → Write XMP Sidecars** writes a standard `.xmp` sidecar next to each selected photo containing its rating, label, title, caption, creator, copyright, and keywords. It's readable by Lightroom, Bridge, Capture One, and exiftool, and **never modifies the original image file**.
- **Photo → Export Metadata (CSV)…** writes a spreadsheet of the selected photos' (or the whole current view's) metadata — an app-independent record for archival or research handoff.
