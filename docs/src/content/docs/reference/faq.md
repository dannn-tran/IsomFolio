---
title: FAQ
description: Frequently asked questions about IsomFolio.
---

## General

### Is IsomFolio free?

Yes. IsomFolio is open-source and free to use.

### Does it require an internet connection?

No. The core app is fully offline. The autotag-openai extension is the only component that makes network requests, and it requires an explicit API key — it does not run automatically.

### Will my photos be uploaded anywhere?

Never, unless you deliberately use the autotag-openai extension (which sends thumbnails to OpenAI). All other processing — thumbnails, CLIP tagging, face clustering — happens locally.

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

If photos move to a new path, IsomFolio marks them as orphaned. Use **File → Rescan** on the affected folder to update paths. IsomFolio does not automatically track file moves — a file moved is treated as a deletion at the old path and a new file at the new path.

---

## AI & extensions

### Do I need any AI extensions to use IsomFolio?

No. All core features — browsing, culling, tagging, albums, search — work without any extensions.

### Is GPU required for AI features?

No. Both autotag-clip and the face-clustering extension run on CPU. GPU acceleration is not currently implemented. CPU-based CLIP tagging is reasonably fast; face clustering on very large libraries can be slow.

### How accurate is the AI tagging?

autotag-clip accuracy varies by subject. It performs well on common subjects (people, animals, landscapes, objects) and less well on abstract or highly specific content. Confidence scores help filter out weak matches. You always review suggestions before they're confirmed.

### Can I run multiple AI extensions?

Yes. You can install and run both autotag-clip and autotag-openai. Use the **preferred extension** setting to choose which one runs automatically on new imports. The other remains available for manual runs.

---

## Photos & metadata

### Can I tag multiple photos at once?

Yes. Select multiple photos, open the Info panel (`I`), and any tags, ratings, or flags you set apply to all selected photos.

### Can I undo an accidental rating or tag change?

Yes. `Cmd+Z` undoes the last operation. The undo history covers tag edits, rating changes, and flag changes for the current session.

### Does IsomFolio support RAW files?

Not yet. JPEG, PNG, WebP, and GIF are supported. RAW support is planned.

### What is a "pending tag"?

A pending tag is an AI suggestion that hasn't been confirmed yet. It appears separately from regular tags in the Info panel with accept/reject controls. Pending tags are not included in search results until accepted.

---

## Performance

### How fast is thumbnail generation?

IsomFolio generates thumbnails in parallel using a pool of background threads. On a modern Mac with an M-series chip, a 1,000-photo import typically completes in under a minute. The first scan is the slowest — subsequent launches load from cache and are instant.

### My library has 50,000+ photos. Will it be slow?

The grid view uses a virtualised renderer — only visible tiles are in memory at any time. Browsing large libraries is fast regardless of library size. Database queries use indexes and should return in milliseconds. Face clustering on 50,000 photos may take 30–60 minutes on first run.

### IsomFolio is using a lot of CPU at idle. Why?

It should not be. IsomFolio's thumbnail pool blocks when idle and only wakes when new work arrives. If you observe sustained CPU usage at idle, please file an issue with your macOS version and library size.
