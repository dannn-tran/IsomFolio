---
title: Searching & Filtering
description: Use the search bar and criteria panel to find photos by text, tags, ratings, flags, dates, and file types.
---

IsomFolio offers two ways to narrow down your library: the **search bar** for quick text search, and the **criteria panel** for structured multi-criteria filtering.

## Quick search

The search bar runs along the top of the grid. Type any text to instantly filter to matching photos. Search matches against:

- File names and folder paths
- Tags
- **Descriptive metadata** — title, caption, creator, and subjects

Results update as you type (with a brief debounce). A single word does prefix matching ("harb" finds "harbour"). Type more than one word and the search becomes a full expression:

- Multiple words require **all** of them (`fishing boats`)
- `OR` and `NOT` — `boats OR nets`, `fishing NOT nets`
- `"exact phrases"` in quotes
- field filters — `tags:portrait`, `filename:DSC`, `folder:2024`

Clear the search field to return to all photos.

## Criteria panel

Click the filter icon in the toolbar to open the criteria panel. It supports:

| Criterion | Options |
|---|---|
| **Tags** | Add tag chips, then choose how they combine with the **All / Any** toggle: **All** = a photo must have every tag (AND), **Any** = at least one (OR). Click a chip to flip it to **exclude** (shown as `−tag` in red) — photos with that tag are dropped (NOT). The `×` removes a chip entirely. A tag also matches its sub-tags (adding `Subject` matches `Subject/Arnold`). |
| **Date from / to** | Filter by capture date (EXIF) or file date within a range. Quick presets — **Last 7 days**, **Last 30 days**, **This month**, **This year** — fill the range for you |
| **File types** | Toggle individual extensions (JPEG, PNG, WebP, GIF) |
| **Rating** | A comparator (**≥ / = / ≤**) with a star count, plus **Unrated** (your review queue) and **Any** — so "≥ 3", "exactly 2", "≤ 1", or "unrated only" |
| **Flag** | Toggle any combination of **Picks / Unflagged / Rejects** (OR) — e.g. Picks + Unflagged = everything not rejected |
| **Location** | Any / With GPS / Without GPS |
| **Person** | Restrict to a named face cluster — appears once you have named people. Combine with tags/dates for "photos of Maya tagged portrait in 2023" |
| **Added** | Any / last 7 days / last 30 days, by when the photo entered the catalog ("what's new") |
| **Camera** | Restrict to one EXIF camera model — appears once your library has camera metadata |

All criteria combine with AND logic — a photo must match every active criterion to appear.

### Combining with quick search

The text search bar and criteria panel work together. You can search for "paris" while also filtering to 4+ stars and tag "street" — all three conditions apply simultaneously.

## Sort order

Sort controls appear in the toolbar:

- **Sort field** — a dropdown: Name, Date Shot, Size, Type
- **Sort direction** — ascending or descending toggle

Sorting and filtering are independent — you can sort any filtered result set.

## Saving a search as a smart album

Any criteria combination can be saved as a **smart album**:

1. Configure your criteria in the criteria panel.
2. Click **Save as Smart Album** at the bottom of the panel.
3. Name the album.

The smart album appears in the sidebar and re-evaluates its criteria every time you view it. See [Albums & Smart Albums](/guide/albums/) for more.

## Search scope

Search and filter always apply to the **current sidebar selection**. If you've selected a specific folder or album, search filters within that context — not the entire library. Switch to **All Files** to search across everything.
