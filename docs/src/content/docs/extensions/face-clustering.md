---
title: Face Clustering Extension
description: Automatically detect and group faces into named people using the faces inference engine.
---

import { Aside, Steps } from '@astrojs/starlight/components';

The **faces extension** detects faces in your photos and groups them into people — one group per person. Everything runs locally.

It ships as a small **inference engine**: a local HTTP server that turns images into face embeddings using ONNX Runtime. IsomFolio starts it on demand, sends batches of photos, stores the embeddings, and does the grouping itself.

## How it works

1. **Detection & embedding** — the engine detects faces in each photo (bounding boxes) and generates a face embedding (a compact mathematical fingerprint) for each.
2. **Clustering** — IsomFolio groups embeddings by similarity (DBSCAN). Photos of the same person land in the same group.
3. **Storage** — embeddings and group memberships are saved in the catalog database, so re-runs only process new photos.

## Requirements

- ~200 MB disk space for model files (downloaded on first run)
- A reasonable CPU (no GPU required; GPU acceleration is not currently supported)
- The engine binary is self-contained — **no .NET runtime install needed**
- **Intel (x86) Macs only:** Docker Desktop. ONNX Runtime no longer ships an Intel-Mac native library, so on that platform the engine runs in a Linux container. Apple Silicon, Windows, and Linux run it natively.

## Installation

<Steps>

1. Download `faces.isfx` from the releases page (or build it with `scripts/build-faces.sh`).
2. Open **Settings → Extensions → Install Extension…** and select the file.
3. **Intel Mac only:** start Docker Desktop and build the engine image once:
   ```
   ./scripts/build-faces-docker.sh
   ```

</Steps>

<Aside type="note">
There is no separate setup step. The first time you find people, the engine downloads its model weights (~200 MB for the default model) — that run takes a little longer; later runs are fast.
</Aside>

## Settings

Under **Settings → General → Face inference engine**:

| Setting | What it does |
|---|---|
| **Auto** (default) | IsomFolio starts and manages a local engine for you. |
| **Custom URL** | Point at an engine you host yourself (e.g. a Docker container or a GPU box on your LAN). Anything implementing the `/health` + `/embed` contract works. |
| **Port** | Port the managed local engine binds (default `45876`, localhost only). |
| **Sensitivity** | Lower groups only very similar faces (stricter); higher groups more loosely. Default `0.4`. |
| **Min faces per person** | Smallest group size that counts as a person. Default `2`. |

The model size (`buffalo_l` high accuracy vs `buffalo_s` smaller/faster) is set on the engine entry in **Settings → Extensions**.

## Automatic clustering on sync

By default, people detection runs automatically after each sync that finds new photos — no manual trigger needed. Turn it off under **Settings → General → Auto-detect people**.

When disabled, trigger it manually:

<Steps>

1. Make sure your library is scanned and up to date.
2. Go to **Extensions → Run Face Clustering** (or use the People view header button).
3. A progress bar appears while photos are embedded, then grouped. Only new/changed photos are embedded.
4. When complete, the People view populates.

</Steps>

<Aside type="tip">
If results look wrong (one person split across groups, or two people merged), try **Run Face Clustering (Full)** from the Extensions menu — it re-groups every face from scratch instead of only assigning new faces to known people.
</Aside>

## Incremental vs full clustering

| Mode | What it processes | When to use |
|---|---|---|
| **Incremental** (default) | Embeds new/changed photos, assigns faces to existing people | Regular use after adding new photos |
| **Full re-cluster** | Re-groups every face from scratch | After bulk imports, or if results are inaccurate |

## Working with people

See [People (Face Recognition)](/guide/people/) for the full UI workflow — naming people, merging groups, removing mis-grouped photos.

## Accuracy notes

- Works best on faces ≥ 80px wide in the original image
- Accuracy improves with more photos of each person
- Identical twins will likely be grouped together
- Faces at extreme angles or partially obscured may not be detected

## Known limitations

<Aside type="note" title="Implementation status">
- **No GPU acceleration** — embedding a large library (10,000+ photos) can take 10–30 minutes on CPU the first time. Subsequent runs only process new photos.
- **Photos with no detected faces** are re-checked on each run (they leave no embedding to cache against).
</Aside>
