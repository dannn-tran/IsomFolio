---
title: Searching & Filtering
description: Use the search bar and criteria panel to find photos by text, tags, ratings, flags, dates, and file types.
---

IsomFolio offers two ways to narrow down your library: the **search bar** for quick text search, and the **criteria panel** for structured multi-criteria filtering.

## Quick search

The search bar runs along the top of the grid. Type any text to instantly filter to matching photos. Search matches against:

- File names
- Tags
- Folder paths

Results update as you type (with a brief debounce to avoid unnecessary database queries).

Clear the search field to return to all photos.

## Criteria panel

Click the filter icon in the toolbar to open the criteria panel. It supports:

| Criterion | Options |
|---|---|
| **Tags** | Include photos tagged with all specified tags |
| **Date from / to** | Filter by file date within a range |
| **File types** | Toggle individual extensions (JPEG, PNG, WebP, GIF) |
| **Rating** | Minimum star rating (≥ 1 through ≥ 5) |
| **Flag** | All / Picks only / Rejects only / Unflagged only |

All criteria combine with AND logic — a photo must match every active criterion to appear.

### Combining with quick search

The text search bar and criteria panel work together. You can search for "paris" while also filtering to 4+ stars and tag "street" — all three conditions apply simultaneously.

## Sort order

Sort controls appear in the toolbar:

- **Sort field** — cycle through Name, Date, Rating
- **Sort direction** — ascending or descending

Sorting and filtering are independent — you can sort any filtered result set.

## Saving a search as a smart album

Any criteria combination can be saved as a **smart album**:

1. Configure your criteria in the criteria panel.
2. Click **Save as Smart Album** at the bottom of the panel.
3. Name the album.

The smart album appears in the sidebar and re-evaluates its criteria every time you view it. See [Albums & Smart Albums](/guide/albums/) for more.

## Search scope

Search and filter always apply to the **current sidebar selection**. If you've selected a specific folder or album, search filters within that context — not the entire library. Switch to **All Files** to search across everything.
