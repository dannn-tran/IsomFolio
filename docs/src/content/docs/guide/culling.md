---
title: Culling — Ratings & Flags
description: Use ratings, pick/reject flags, and filters to efficiently cull a shoot.
---

import { Aside } from '@astrojs/starlight/components';

**Culling** is the process of sorting through photos after a shoot — identifying the keepers, rejecting the failures, and ranking the best. IsomFolio is built for fast keyboard-driven culling.

## Flags

Every photo has one of three flag states:

| Flag | Key | Meaning |
|---|---|---|
| **Pick** | `P` | This is a keeper |
| **Reject** | `X` | Delete or skip — not worth keeping |
| **Unflagged** | `U` | Default; not yet reviewed |

Flags apply to the **selected photo(s)**. Select multiple photos with Cmd+Click or Cmd+A, then press a flag key to apply to all.

### Workflow tip

A common culling workflow:
1. Go through all photos with `→`, pressing `X` on clear rejects and `P` on strong keepers.
2. Press `\` to hide rejects — the grid now shows only unflagged and picks.
3. Rate the picks with `1`–`5`.

## Ratings

Ratings are 0–5 stars (0 = no rating).

| Key | Rating |
|---|---|
| `0` | Clear rating |
| `1` | 1 star |
| `2` | 2 stars |
| `3` | 3 stars |
| `4` | 4 stars |
| `5` | 5 stars |

<Aside type="caution" title="Known limitation">
Ratings are stored and filterable but are **not yet displayed as overlays on grid tiles**. You can see a photo's rating in the Info panel. Visual overlays on tiles are planned.
</Aside>

## Filtering by flag or rating

Open the **criteria panel** (filter button in the toolbar or the criteria toggle) to filter the current view by:

- Flag: All / Picks / Rejects / Unflagged
- Minimum rating: show only photos with ≥ N stars

These filters combine with text search and tag filters.

## Undo

All rating and flag changes are undoable.

- `Cmd+Z` — undo
- `Cmd+Shift+Z` — redo

The undo history is preserved for the current session.

## Batch culling

Select multiple photos (Cmd+A to select all, or Cmd+Click for individual selection) and apply a flag or rating — it applies to all selected photos simultaneously. This is useful for quickly flagging an entire burst as rejected.
