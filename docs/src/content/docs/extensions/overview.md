---
title: What are Extensions?
description: How the IsomFolio extension system works and why AI capabilities are opt-in.
---

import { Aside } from '@astrojs/starlight/components';

IsomFolio's extension system lets you add AI capabilities — currently **face recognition** — without bundling those dependencies into the core app.

## Why extensions?

AI libraries (face detection and recognition models, neural-network runtimes) are large and slow to install, and not every user needs them. IsomFolio keeps the core app lean and fast, and lets you opt in to exactly the AI you want.

## How it works

The face extension ships as an **inference engine**: a small local HTTP server that turns images into face embeddings. IsomFolio starts it on demand, sends batches of photos, stores the results, and does the grouping itself. Nothing leaves your machine.

```
~/.local/share/IsomFolio/extensions/
└── faces/
    ├── manifest.json   ← capabilities + config schema
    └── faces           ← the inference engine executable
```

On Apple Silicon, Windows, and Linux the engine runs natively. On Intel Macs it runs in a Docker container (the underlying ONNX Runtime no longer ships an Intel-Mac native build) — see [Face Clustering](/extensions/face-clustering/).

## The manifest

Every extension ships a `manifest.json` that tells IsomFolio:
- What capability it provides (`inference_engine`)
- What configuration fields it exposes (e.g. model size)

## Installing an extension

Extensions are distributed as `.isfx` files — zip archives with a specific layout. Install one via **Settings → Extensions → Install Extension**.

See [Installing Extensions →](/extensions/installing/)

## Security model

<Aside type="caution">
Extensions are executables that run on your machine. Only install extensions from sources you trust.
</Aside>

IsomFolio takes these precautions when installing:
- Zip archives are extracted with path-traversal protection (no `../../` escape attacks)
- Executables are set to the minimum required permissions
- The engine binds to localhost only and is torn down when the app quits

## Available extensions

| Extension | Capability | Language |
|---|---|---|
| [faces](/extensions/face-clustering/) | Face detection + embeddings (HTTP inference engine) | C# + ONNX Runtime |
