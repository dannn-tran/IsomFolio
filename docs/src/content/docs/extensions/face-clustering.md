---
title: Face Clustering Extension
description: Automatically detect and group faces into named people using the faces extension.
---

import { Aside, Steps } from '@astrojs/starlight/components';

The **faces extension** detects faces in your photos and groups them into clusters — one cluster per person. Everything runs locally using ML.NET.

## How it works

1. **Detection** — the extension scans each photo and detects faces, recording their bounding boxes and generating face embeddings (compact mathematical representations of each face).
2. **Clustering** — embeddings are grouped by similarity. Photos of the same person end up in the same cluster.
3. **Storage** — cluster memberships are saved to the catalog database.

## Requirements

- .NET 8 runtime
- ~200 MB disk space for model files
- Reasonable CPU (no GPU required; GPU acceleration is not currently supported)

## Installation

<Steps>

1. Download `faces.isfx` from the releases page.
2. Open **Settings → Extensions → Install Extension…** and select the file.
3. The setup step downloads face detection model weights. This may take a minute.

</Steps>

## Automatic clustering on sync

By default, face clustering runs automatically after each sync that finds new photos — no manual trigger needed. You can turn this off in **Settings → Behaviour → Auto face clustering**.

When disabled, trigger clustering manually:

<Steps>

1. Make sure your library is scanned and up to date.
2. Go to **Extensions → Run Face Clustering** (or use the People view header button).
3. A progress bar appears. Clustering is incremental by default — only changed photos are re-processed.
4. When complete, the People view populates with clusters.

</Steps>

<Aside type="tip">
If clustering results look wrong (people split across multiple clusters, or two people merged), try **Run Face Clustering (Full)** from the Extensions menu. This re-processes all photos from scratch.
</Aside>

## Incremental vs full clustering

| Mode | What it processes | When to use |
|---|---|---|
| **Incremental** (default) | Only new/changed photos since last run | Regular use after adding new photos |
| **Full re-cluster** | All photos | After bulk imports, or if results are inaccurate |

## Working with clusters

See [People (Face Recognition)](/guide/people/) for the full UI workflow — naming people, merging clusters, removing mis-clustered photos.

## Accuracy notes

- Works best on faces ≥ 80px wide in the original image
- Accuracy improves with more photos of each person
- Identical twins will likely be grouped in the same cluster
- Faces at extreme angles or partially obscured may not be detected

## Known limitations

<Aside type="note" title="Implementation status">
The face clustering extension is functional. The following limitations are known:

- **No GPU acceleration** — clustering on large libraries (10,000+ photos) can take 10–30 minutes on CPU
</Aside>
