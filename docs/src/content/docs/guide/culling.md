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
1. Go through all photos with `→`, pressing `X` on clear rejects and `P` on strong keepers. Rejected photos **dim in place** rather than disappearing, so the grid keeps its order and a reject is one keypress (`U` or `P`) from coming back.
2. When you want them gone from view entirely, press `\` to hide rejects — the grid then shows only unflagged and picks.
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

## Clearing rejects

Pressing **Delete** (or **Backspace**) on the selected photos, or **Edit → Delete Rejected Photos…**, moves them to the **Deleted** folder. This is a *virtual* delete: **your files on disk are never moved or touched** — the photos just drop out of all normal views and collect in a "Deleted" entry that appears in the sidebar.

To **recover**, open the **Deleted** folder, select the photos, and choose **Restore** (right-click) — they return to their place instantly, with ratings and tags intact. (Inside a manual album, Delete instead just removes the photos from that album.)

To remove them for good, use **Delete Permanently…** (right-click) on a selection, or **Empty Deleted…** in the status bar. This is the *only* action that deletes the actual files from disk, so it asks for confirmation and **cannot be undone**.

## Colour labels

A second, independent axis for organising a cull (use stars for quality, colours for status — e.g. "to retouch", "client pick", "social"):

| Key | Colour |
|---|---|
| `6` | Red |
| `7` | Yellow |
| `8` | Green |
| `9` | Blue |

Press a colour key again to clear it. Purple (and any colour) can also be set from the swatches in the Loupe overlay. Colour shows as a dot on the grid tile and in Loupe, and is a filter in the cull strip. Labels are stored as the standard XMP `xmp:Label`.

## Filtering by flag or rating

The **cull strip** sits directly under the toolbar and is always visible — no need to open a panel:

- **Flag** — toggle any combination of **Picks**, **Unflagged**, and **Rejects**. They combine as "OR", so you can show, for example, *Picks + Unflagged* (everything you haven't rejected). With none (or all three) selected, the flag filter is off. The toolbar **Hide Rejects** chip (and the `\` key) is a one-tap shortcut for the *Picks + Unflagged* combination.
- **Stars** — pick a comparator (**≥**, **=**, **≤**) and a star count, or choose **Unrated** to find photos you haven't rated yet (your review queue), or **Any** to clear it. So "3 stars or more", "exactly 2", "1 or fewer", and "unrated only" are all one or two clicks.

Advanced filters (tags, date, type, location, person, camera) live in the **Filters** panel (`F` or the Filters button). All filters combine with text search.

## Undo

All rating and flag changes are undoable.

- `Cmd+Z` — undo
- `Cmd+Shift+Z` — redo

The undo history is preserved for the current session.

## Batch culling

Select multiple photos (Cmd+A to select all, or Cmd+Click for individual selection) and apply a flag or rating — it applies to all selected photos simultaneously. This is useful for quickly flagging an entire burst as rejected.
