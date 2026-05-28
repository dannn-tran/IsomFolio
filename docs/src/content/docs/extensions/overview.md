---
title: What are Extensions?
description: How the IsomFolio extension system works and why AI capabilities are opt-in.
---

import { Aside } from '@astrojs/starlight/components';

IsomFolio's extension system lets you add AI capabilities — auto-tagging, face recognition, and more — without bundling those dependencies into the core app.

## Why extensions?

AI libraries (CLIP models, face detection, neural networks) are large, slow to install, and often require specific Python environments or GPU drivers. Not every user needs them. IsomFolio keeps the core app lean and fast, and lets you opt in to exactly the AI you want.

## How extensions work

Extensions are **separate executables** that IsomFolio launches as child processes. They communicate over a simple JSON protocol on stdin/stdout. They can be written in any language — the bundled extensions are written in Rust and Python.

```
~/.local/share/IsomFolio/extensions/
├── autotag-clip/
│   ├── manifest.json      ← describes capabilities and config schema
│   └── autotag-clip       ← the executable
└── faces/
    ├── manifest.json
    └── faces
```

## The manifest

Every extension ships a `manifest.json` that tells IsomFolio:
- What capabilities it provides (`classify`, `cluster_faces`, etc.)
- What configuration fields it needs (API keys, model paths, thresholds)
- Whether it needs a setup step on first install

## Installing an extension

Extensions are distributed as `.isfx` files — which are zip archives with a specific layout. Install one via **Settings → Extensions → Install Extension**.

See [Installing Extensions →](/extensions/installing/)

## Security model

<Aside type="caution">
Extensions are executables that run on your machine. Only install extensions from sources you trust.
</Aside>

IsomFolio takes these precautions when installing:
- Zip archives are extracted with path traversal protection (no `../../` escape attacks)
- Executables are set to the minimum required permissions
- Each extension runs in its own child process, isolated from the main app

## Writing your own extension

The extension protocol is documented in the [`isfx-sdk`](https://github.com/picas9dan/isomfolio/tree/main/extensions/isfx-sdk) crate. A minimal extension in Rust:

```rust
use isfx_sdk as sdk;

fn main() {
    sdk::run(|request| {
        match request.method.as_str() {
            "classify" => {
                // process request.params, return tags
                sdk::Response::ok(serde_json::json!({
                    "file_id": request.params["file_id"],
                    "tags": [{ "tag": "my-tag", "confidence": 0.9 }]
                }))
            }
            _ => sdk::Response::error("unknown method"),
        }
    });
}
```

## Available extensions

| Extension | Capability | Language |
|---|---|---|
| [autotag-clip](/extensions/autotag-clip/) | Auto-tagging via CLIP embeddings | Rust + ONNX |
| [autotag-openai](/extensions/autotag-openai/) | Auto-tagging via OpenAI Vision API | Rust |
| [faces](/extensions/face-clustering/) | Face detection + clustering | C# + ML.NET |
