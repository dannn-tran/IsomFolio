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

Pressing **Delete** (or **Backspace**) on the selected photos, or **Edit → Delete Rejected Photos…**, moves them to the **Deleted** folder. This is a *virtual* delete: **your files on disk are never moved or touched** — the photos just drop out of all normal views and collect in a "Deleted" entry that appears in the sidebar. In the loupe, **Delete** removes the photo you're viewing and slides the next one into place, so you can keep culling without leaving the loupe. The first time you delete, a one-time status note reminds you that your files on disk are untouched.

To **recover**, open the **Deleted** folder, select the photos, and choose **Restore** (right-click) — they return to their place instantly, with ratings and tags intact. (Inside a manual album, Delete instead just removes the photos from that album.)

To remove them from your library, use **Move to Trash…** (right-click) on a selection, or **Empty Deleted…** in the status bar. This is the *only* action that touches the files on disk: it moves them to your system **Trash** (Recycle Bin on Windows) — so they're still recoverable there until you empty it — and drops them from the catalog. The files come back, but their **ratings and tags do not** (re-importing gives a fresh entry), so it asks for confirmation first.

## Colour labels

A second, independent axis for organising a cull (use stars for quality, colours for status — e.g. "to retouch", "client pick", "social"):

| Key | Colour |
|---|---|
| `6` | Red |
| `7` | Yellow |
| `8` | Green |
| `9` | Blue |

Press a colour key again to clear it. Purple (and any colour) can also be set from the swatches in the Loupe overlay. Colour shows as a dot on the grid tile and in Loupe, and is a filter in the cull strip. Labels are stored as the standard XMP `xmp:Label`.

## Auto-advance

In the **Loupe**, applying any cull verdict — a flag (`P`/`X`/`U`), a rating (`1`–`5`), or a colour label (`6`–`9`) — moves you to the next photo automatically, so a one-handed pass is one keypress per frame. Toggle it under **Settings → General → "Auto-advance after culling"** (on by default). In the grid, multi-photo edits stay put so you can keep refining the same selection.

## Filtering by flag or rating

The **cull strip** sits directly under the toolbar and is always visible — no need to open a panel:

- **Flag** — toggle any combination of **Picks**, **Unflagged**, and **Rejects**. They combine as "OR", so you can show, for example, *Picks + Unflagged* (everything you haven't rejected). With none (or all three) selected, the flag filter is off. The toolbar **Hide Rejects** chip (and the `\` key) is a one-tap shortcut for the *Picks + Unflagged* combination.
- **Stars** — pick a comparator (**≥**, **=**, **≤**) and a star count, or choose **Unrated** to find photos you haven't rated yet (your review queue), or **Any** to clear it. So "3 stars or more", "exactly 2", "1 or fewer", and "unrated only" are all one or two clicks.

Advanced filters (tags, date, type, location, person, camera) live in the **Filters** panel (`F` or the Filters button). All filters combine with text search.

## Gathering keepers with a target album

To collect picks into an album as you go, right-click a manual album → **Set as Target Album** (it's marked `◎` in the sidebar). Then press **`B`** on any selection to add it to that album — no dragging. Press the menu item again to clear the target.

## Undo

Almost everything you do to photos is undoable: ratings, flags, colour labels, tags, **deleting** (and restoring), and **adding to or removing from an album**.

- `Cmd+Z` — undo
- `Cmd+Shift+Z` — redo

Undo follows you back to the photo. In the loupe, flagging or deleting auto-advances to the next frame — pressing `Cmd+Z` reverses the change *and* returns the view to the photo you were on, so a misfire never loses your place. In the grid, the affected photos are re-selected.

The **Edit** menu names the next step — *Undo Rating*, *Redo Delete* — and greys the item out when there's nothing to undo. The history is preserved for the current session.

## Batch culling

Select multiple photos (Cmd+A to select all, or Cmd+Click for individual selection) and apply a flag or rating — it applies to all selected photos simultaneously.


## Comparing similar shots

When you've shot several near-identical frames — a burst, or a few tries at the same setup — pick the best by reviewing them together. Sort the grid by **capture time** (the default) so a burst sits as one contiguous run, then select the candidates: click the first and **Shift+Click** the last to grab the whole run, or **Cmd+Click** individual frames.

With a selection of two or more, the **review surface** opens over just those frames, **side by side**. Open it with **`C`**, **`Space`**, or right-click the selection → **Compare**. It's **one surface with two layouts** you flip between with **`Space`** — all at once, or one big at a time.

### Two layouts, one review

- **Survey — all at once (the default).** The frames sit side by side. Switch between **▭ Row** (a single horizontal line) and **▦ Grid** (wrapped into rows) with the toggle in the top bar — both fit the window, no scrolling either way. **Synced zoom:** scroll or click to zoom into a detail — the subject's eye, say — and *every* frame zooms and pans to the same spot together, so you compare sharpness and expression at 100% across all of them simultaneously.
- **One-up — one big + filmstrip.** The focused frame fills the view over a filmstrip of the set (click any thumb to jump to it). Zoom in to pixel-check focus; turn on **⊞ Lock zoom** and the zoom holds as you step between frames — flicking back and forth becomes a *blink comparison* of the same region.

Switch any time with **`Space`** — same set, same focused frame, just a different view.

### Pick the best, with sharpness guidance

- **Sharpness ranking.** Each frame shows its place (**Sharp #2 / 5**), and the clear winner is marked **◉ Sharpest** — a *relative* cue among the frames you're comparing (never an absolute "blurry" verdict). Toggle **◉ Sharpest first** to reorder sharpest-to-softest. Eyes-open and expression are still your call.
- **Flag right here.** One frame is *focused* (ringed); **`←`** / **`→`** move the focus (or click a Survey pane / filmstrip thumb). The cull keys act on the focused frame — **`P`** picks, **`X`** rejects, **`U`** clears, **`1`–`5`** rate — its badge updates immediately, and with auto-advance on the focus steps to the next frame so you cull a burst with repeated `P`/`X`.
- **Whittle down with `R`.** Press **`R`** to drop the focused frame from the comparison — not a reject, just removes it from the set so the survey narrows to the real contenders. Emptying the set returns you to the grid.

**`Esc`** leaves the review with your whole reviewed set still selected in the grid. The review leaves no trace of its own — your **flags** are the durable result. Collect the keepers into an album with **`B`** (see *Gathering keepers with a target album* above) if you want them grouped.
