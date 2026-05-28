---
title: Your First Catalog
description: Create a catalog, add photos, and get oriented in IsomFolio.
---

import { Steps, Aside } from '@astrojs/starlight/components';

A **catalog** is how IsomFolio organises your photos. Think of it as a project file — it stores all your metadata (tags, ratings, flags, albums) and a thumbnail cache, but it never moves or copies your original photos.

## Create a catalog

<Steps>

1. Launch IsomFolio. The Welcome screen appears.

2. Click **New Catalog…** in the top-right corner.

3. Enter a name (e.g. `My Photos`) and choose a location for the catalog directory.

4. Click **Create**. IsomFolio creates a `My Photos.isfcatalog` directory and opens it immediately.

</Steps>

## Add a folder of photos

<Steps>

1. Click **Add Folder** in the toolbar (or use the **File** menu).

2. Navigate to a folder of photos and click **Open**.

3. IsomFolio scans the folder, generates thumbnails, and adds the photos to the library. A progress indicator appears in the status bar.

</Steps>

<Aside type="tip">
You can add multiple folders. Each appears as a separate entry in the sidebar. IsomFolio watches all added folders for new, changed, and deleted files — changes appear automatically without a manual rescan.
</Aside>

## What you'll see

After scanning, the **grid view** fills with your photos. From here you can:

- Click a photo to select it and see metadata in the **Info panel** (press `I` to toggle)
- Press `Space` to open the **Loupe** (full-screen single-photo view)
- Press `P` / `X` to flag photos as **Pick** / **Reject**
- Press `1`–`5` to set a **star rating**

## Open an existing catalog

On the Welcome screen, recent catalogs appear in the list on the left. Click one to select it, then click **Open**. You can also drag a `.isfcatalog` directory onto the window.

## Where data is stored

```
My Photos.isfcatalog/
├── catalog.db        ← SQLite database: all metadata
└── thumbnails/       ← JPEG thumbnail cache
```

App preferences (recent catalogs, extension settings) are stored in `~/Library/Application Support/IsomFolio/`.

<Aside type="note">
Your original photos are never touched. IsomFolio reads them to generate thumbnails and extract metadata, but never modifies, moves, or copies them.
</Aside>
