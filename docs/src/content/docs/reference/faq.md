---
title: FAQ
description: Frequently asked questions about IsomFolio.
---

## General

### Is IsomFolio free?

Yes. IsomFolio is open-source and free to use.

### Does it require an internet connection?

No, for everyday use. The only network access is a one-time download of the face-recognition models (~200 MB) the first time you use the face extension. Everything else — browsing, thumbnails, tagging, face clustering — is fully offline.

### Will my photos be uploaded anywhere?

Never. All processing — thumbnails, face detection, and clustering — happens locally on your machine.

### Does IsomFolio modify my original photos?

No. IsomFolio reads your photos to generate thumbnails and extract metadata, but never writes to the original files. All metadata (tags, ratings, flags) is stored in the catalog database.

---

## Catalog & storage

### Can I have multiple catalogs?

Yes. Each catalog is an independent directory. You can switch between them from the Welcome screen or File menu. Only one catalog is open at a time.

### Can I share a catalog between computers?

Catalogs use absolute file paths internally, so sharing a catalog between machines with different filesystem layouts requires path adjustments. Syncing via iCloud Drive or Dropbox may work if the folder structure is identical on both machines, but this is not officially tested.

### How large is a catalog?

The database is typically small (a few MB even for large libraries). The thumbnail cache is the main storage consumer — approximately 20–100 KB per photo depending on image size and thumbnail quality. A 10,000-photo library has a thumbnail cache of roughly 200 MB–1 GB.

### Can I delete the thumbnail cache?

Yes. Delete `MyPhotos.isfcatalog/thumbnails/` and IsomFolio will regenerate thumbnails on next launch. This takes time but does not affect your metadata.

### What happens if I move my photos?

When files are added, removed, moved, or renamed in a watched folder, IsomFolio flags that folder with a dot in the sidebar — it does **not** change your catalog automatically. Sync the folder (`Cmd+R` or right-click → Sync Folder) to apply the changes: new files are indexed, missing files are marked orphaned. A move is treated as a deletion at the old path and a new file at the new path; use **Locate…** to reconnect a moved file and keep its ratings and tags. (Editing a photo's pixels in place — same path — just refreshes its thumbnail; nothing to sync.)

---

## AI & extensions

### Do I need any AI extensions to use IsomFolio?

No. All core features — browsing, culling, tagging, albums, search — work without any extensions.

### Is GPU required for AI features?

No. The face extension runs entirely on CPU. GPU inference is not currently supported. The first run embeds your whole library (which can take a while on large collections); later runs only process new photos.

---

## Photos & metadata

### Can I tag multiple photos at once?

Yes. Select multiple photos, open the Info panel (`I`), and any tags, ratings, or flags you set apply to all selected photos.

### Can I undo an accidental rating or tag change?

Yes. `Cmd+Z` undoes the last operation. The undo history covers tag edits, rating changes, and flag changes for the current session.

### Does IsomFolio support RAW files?

Not yet. JPEG, PNG, WebP, and GIF are supported. RAW support is planned.

---

## Performance

### How fast is thumbnail generation?

IsomFolio generates thumbnails in parallel using a pool of background threads. On a modern Mac with an M-series chip, a 1,000-photo import typically completes in under a minute. The first sync is the slowest — subsequent launches load from cache and are instant.

### My library has 50,000+ photos. Will it be slow?

The grid view uses a virtualised renderer — only visible tiles are in memory at any time. Browsing large libraries is fast regardless of library size. Database queries use indexes and should return in milliseconds. Face clustering on 50,000 photos may take 30–60 minutes on first run.

### IsomFolio is using a lot of CPU at idle. Why?

It should not be. IsomFolio's thumbnail pool blocks when idle and only wakes when new work arrives. If you observe sustained CPU usage at idle, please file an issue with your macOS version and library size.
