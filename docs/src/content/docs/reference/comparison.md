---
title: Comparison
description: How IsomFolio compares to Lightroom, Apple Photos, Capture One, and Digikam.
---

import { Aside } from '@astrojs/starlight/components';

<Aside type="note">
This comparison is based on publicly available information as of 2025. Feature parity changes over time.
</Aside>

## Feature comparison

| Feature | IsomFolio | Lightroom | Apple Photos | Capture One | digiKam |
|---|---|---|---|---|---|
| **Price** | Free / open-source | $10–22/mo | Free (with Apple hardware) | $24/mo or $299 perpetual | Free / open-source |
| **Local-first** | ✅ | Partial (Classic) | Partial (iCloud sync) | ✅ | ✅ |
| **No subscription** | ✅ | ❌ | ✅ (but ecosystem lock-in) | Partial (perpetual available) | ✅ |
| **RAW support** | 🔶 Planned | ✅ | ✅ | ✅ | ✅ |
| **Non-destructive editing** | ❌ | ✅ | ✅ | ✅ | Partial |
| **Star ratings** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Pick/reject flags** | ✅ | ✅ | ❌ | ✅ | ✅ |
| **Tags / keywords** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Smart albums/folders** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Face recognition** | ✅ (opt-in, local) | ✅ (cloud) | ✅ (on-device) | ❌ | ✅ (local) |
| **Open data format** | ✅ (SQLite) | ❌ (proprietary) | ❌ (proprietary) | ❌ (proprietary) | ✅ (SQLite) |
| **Extension system** | ✅ | ❌ | ❌ | ❌ | Partial (plugins) |
| **Offline AI** | ✅ | ❌ | ✅ (limited) | ❌ | Partial |
| **Linux support** | ✅ (beta) | ❌ | ❌ | ❌ | ✅ |

## Why choose IsomFolio?

**Choose IsomFolio if:**
- You want your photo metadata in an open, inspectable format (SQLite) with no lock-in
- You want local face recognition without sending photos to any cloud
- You want a fast, keyboard-driven culling workflow without paying a subscription
- Privacy is a non-negotiable requirement

**Consider alternatives if:**
- You need RAW processing and editing (use Lightroom or Capture One)
- You need deep Apple ecosystem integration (iCloud, Memories, Shared Albums) — use Apple Photos
- You need a fully mature, production-hardened tool for professional studio work — IsomFolio is still in active development

## vs Adobe Lightroom Classic

Lightroom Classic is the closest workflow match — it uses a local catalog (`.lrcat`), supports keywording, collections, star ratings, and flags, and has a dedicated culling workflow.

IsomFolio differences:
- **Free** vs $10+/month
- **Open SQLite catalog** vs proprietary `.lrcat`
- **No editing** vs full develop module
- **Local face recognition** vs Adobe Firefly (cloud AI)
- **Extensible** via `.isfx` packages

## vs Apple Photos

Apple Photos integrates deeply with macOS and iOS but uses a proprietary library format. Exporting from Apple Photos is lossy — album structure, ratings, and keywords don't survive well.

IsomFolio differences:
- **Open format** — migrate at any time without data loss
- **No iCloud required** — works fully air-gapped
- **No editing** vs Apple Photos adjustments

## vs digiKam

digiKam is the closest open-source equivalent. It is mature, feature-rich, and cross-platform.

IsomFolio differences:
- **Younger / less features** currently
- **Extension system** for AI — digiKam has built-in AI but it's not as modular
- **Faster UI** — IsomFolio uses a GPU-accelerated renderer (iced/wgpu) vs digiKam's Qt widgets
- **No RAW editing** (yet) — digiKam has full RAW support
