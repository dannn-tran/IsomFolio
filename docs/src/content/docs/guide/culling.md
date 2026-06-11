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

## Culling stacks

When you've shot several near-identical frames, IsomFolio groups them into a **stack** (see [Browsing → Stacks](/guide/browsing/)). When the current library has groups to work through, a **Sift (N)** chip appears in the toolbar — click it (or press **`R`**) to start the guided pass described below. The neighbouring **⧉ Stack** chip instead collapses each stack to one tile in the grid (e.g. `⧉ Stack (12)`); how aggressively frames group is tunable under **Settings → General → Stacking** (similarity threshold + max time gap). With collapse on, you don't have to expand a burst to resolve it:

- **Keep this, reject rest** — right-click the stack's tile. The frame you clicked is flagged **Pick** and every other frame in the stack is flagged **Reject** — the keep-the-best-of-a-burst decision in one action. (The collapsed tile defaults to the *sharpest* frame; expand the stack first by clicking its `⧉ N` badge if you want to keep a different one.)
- **Reject whole stack** — flags every frame in the stack as a Reject (e.g. the whole burst missed).

Both are undoable with `Cmd+Z`, and they apply to every frame in the stack even the ones hidden behind the collapsed tile.

### Sift — a guided pass

When a shoot has *many* bursts, press **`R`** (the **Sift (N)** chip, or **View → Sift Bursts**) to step through them one at a time. This opens a full-screen review — the same kind of focused view as the loupe — showing one group's frames large in an **adaptive grid that fits the window** (no horizontal scrolling, even for a row of landscapes), so you can actually judge focus, eyes, and expression:

1. The **sharpest** frame is pre-marked as the keeper. Every frame shows its sharpness **rank** — `★ sharpest` on the auto-pick, `#2`, `#3`… on the rest — so if you reject the default for a blink or a stray hand, you can see at a glance which of the others is next-sharpest. Click any frame to toggle whether it's kept — kept frames get a blue ring and **✓ Keep**, the rest show **✕ Reject** and dim.
2. Confirm with **Keep selected & Next ›** to flag your choice (keepers → Pick, the rest → Reject) and jump to the next group. **Skip** moves on without changing anything; **‹ Previous** steps back — and your earlier keep/reject choices are remembered, so stepping back and forth never loses them. Keeping *nothing* is treated as a Skip (you can't silently reject a whole group).
3. When you've worked through every group — or press **Esc** to stop early — you're returned to the grid.

**Two layouts.** Toggle **▦ Grid** / **▭ Strip** in the header. *Grid* shows every frame at once in a window-filling adaptive grid — best for small groups. *Strip* shows one large preview of the focused frame over a thumbnail filmstrip — best for many frames (and for pixel-checking focus), so the photo stays big instead of shrinking. Groups with more than four frames open in Strip automatically; pick a layout yourself and it sticks for the rest of the pass.

**Keyboard — the whole pass is one-handed:**

| Key | Action |
|---|---|
| `Enter` or `Space` | Keep selected & Next |
| `←` / `→` | *Grid:* previous / next group · *Strip:* focus previous / next frame |
| `Shift`+`←` / `→` | Previous / next group (either layout) |
| `1`–`9` | Toggle keep on that frame (by position) |
| `0` | Reset to the auto-pick (sharpest) |
| `Esc` | Exit to grid |

If you've toggled away from the auto-pick, a **↺ Reset to auto** button (and `0`) restores it.

Everything you do here is undoable with `Cmd+Z`. It's the fastest way to cull a shoot full of bursts down to one keeper each.

### Sift Scenes — looser groups by content

Bursts are *tight*: near-identical frames shot seconds apart. **Scenes** are looser — "several tries at the same shot or subject" even when you reframed, zoomed, or recomposed between frames. Press **`⇧R`** (or **View → Sift Scenes**) to step through them in the exact same guided, full-screen pass, picking a keeper for each.

The difference is how the groups are formed. Bursts compare pixels (a perceptual hash); scenes compare **image content** (a whole-image embedding), so a group survives a pan or a tighter crop that would split a burst. Use **Sift Bursts** for burst/bracketing cleanup, **Sift Scenes** for "I tried this portrait twelve ways — show me the set so I can keep the best."

Scene grouping runs in the background after a sync (like stacking), so it may lag fresh thumbnails for a moment. Tune it under **Settings → General → Scene grouping**: **Grouping looseness** (higher pulls more varied frames together), **Min neighbours** (1 lets a two-frame scene form), and an **Auto-embed** toggle. The panel shows how many frames have been embedded so far.
