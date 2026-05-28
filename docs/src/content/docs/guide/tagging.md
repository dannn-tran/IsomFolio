---
title: Tagging
description: Add, manage, and search tags — including AI-suggested tags from extensions.
---

import { Aside } from '@astrojs/starlight/components';

Tags are freeform text labels attached to photos. They power smart albums, search, and AI workflows.

## Adding tags manually

1. Select one or more photos.
2. Open the Info panel (`I`).
3. Type in the tag input field. Autocomplete suggests existing tags as you type.
4. Press `Enter` or `Tab` to add the tag.
5. Click the `×` next to a tag to remove it.

All tag edits are undoable (`Cmd+Z`).

## Repeat last tag

Press `.` (period) to apply the last tag you used to the currently selected photo(s). This is useful when tagging multiple photos with the same tag without opening the Info panel each time.

## Batch tagging

Select multiple photos before opening the Info panel. The panel enters **batch mode**:
- Tags shown are common to all selected photos
- Adding a tag adds it to all selected photos
- Removing a tag removes it from all selected photos

## AI-suggested tags (pending tags)

When an auto-tagging extension runs, it produces **pending tags** — AI suggestions that appear separately from confirmed tags. You review them individually:

- **Accept** (`✓`) — adds the tag permanently
- **Reject** (`✗`) — discards the suggestion
- **Accept All** — accepts all pending tags for the selected photo(s) at once
- **Reject All** — discards all pending suggestions

<Aside type="tip">
Pending tags let you use AI suggestions as a starting point without committing to them. You stay in control of your taxonomy.
</Aside>

## Tag hierarchies

Tags support a path-style hierarchy using `/` as a separator:

```
subject/portrait
subject/landscape
location/paris
gear/sony-a7r5
```

The **Tag Browser** (accessible from the View menu) shows your full tag tree, lets you rename tags across all photos, and lets you delete tags entirely.

### Renaming a tag

Renaming a tag in the Tag Browser updates every photo that has that tag — it's a global rename, not a per-photo edit.

## Tag origins

Every tag has an origin:

| Origin | Source |
|---|---|
| **Manual** | Added by you via the Info panel |
| **AI** | Accepted from an AI extension suggestion |

Origin is tracked in the database but is not currently displayed in the UI.

## Tag browser

Open the Tag Browser from the View menu to:
- See all tags used across your library with usage counts
- Search and filter tags
- Rename a tag (updates all photos)
- Delete a tag (removes it from all photos)
