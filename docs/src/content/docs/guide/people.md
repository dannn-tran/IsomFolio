---
title: People (Face Recognition)
description: Automatically group photos by person using the face-clustering extension.
---

import { Aside } from '@astrojs/starlight/components';

<Aside type="note">
People requires the **face-clustering extension** to be installed. See [Extensions → Face Clustering](/extensions/face-clustering/) for setup instructions. The core app ships without this capability.
</Aside>

## How it works

IsomFolio's People feature uses a two-step process:

1. **Detection** — the extension scans your photos and detects faces, recording bounding box coordinates for each.
2. **Clustering** — detected faces are grouped by similarity into clusters. Each cluster represents one person.

The clustering runs locally on your machine. No face data is sent anywhere.

## Running face clustering

1. Install the face-clustering extension (see [Face Clustering](/extensions/face-clustering/)).
2. Choose **Photo → Find People** (clustering also runs automatically after a sync that finds new photos, unless disabled in Settings).
3. A progress indicator appears in the People view header. Clustering on a large library can take several minutes.
4. When complete, the People view shows one card per detected person.

### Full re-cluster vs incremental

By default, clustering is incremental — it processes only new/changed photos and assigns their faces to existing people (discovering new ones). If faces seem wrong or people are split across multiple clusters, run a **full re-cluster** with the **⟳** button in the People section header. Full re-clustering takes longer but produces the most accurate groupings.

## People view

Click **People** in the sidebar to see all clusters. Each card shows:

- A representative face crop
- The person's name (once you assign one)
- Number of photos in the cluster

## Naming a person

Click the `…` on a cluster card (or right-click) and choose **Rename**. Type a name and press `Enter`. Named clusters appear with the name as their label.

## Merging clusters

If the same person appears in two clusters:

1. Right-click one cluster and choose **Merge into…**
2. Select the target cluster.

The source cluster's photos move into the target. This cannot be undone.

## Removing a photo from a cluster

If a face was mis-clustered:

1. Open the cluster to see its photos.
2. Right-click the incorrectly clustered photo and choose **Remove from this person**.

The photo remains in your library — only the face association is removed.

## Browsing a person's photos

Click a cluster card to enter that person's photo view — a filtered grid showing all photos where that person was detected. All standard grid actions (ratings, flags, tagging) work normally here.

## Limitations

- Faces must be reasonably visible and front-facing for reliable detection.
- Very small faces (under ~40px) may be missed.
- Twins or very similar-looking people may end up in the same cluster.
- Running clustering on a first-time install processes all photos, which can be slow for large libraries.
